use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use std::path::PathBuf;
use std::fs;
use zip_extract;

#[tauri::command]
pub async fn install_retroarch(
    app_handle: tauri::AppHandle,
    target_dir: String,
) -> Result<String, String> {
    let url = "https://buildbot.libretro.com/stable/1.20.0/windows/x86_64/RetroArch.zip";
    let dest_path = PathBuf::from(&target_dir).join("RetroArch_download.zip");
    let extract_path = PathBuf::from(&target_dir).join("RetroArch");

    // 1. Download
    println!("Downloading RetroArch...");
    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&dest_path, bytes).map_err(|e| e.to_string())?;

    // 2. Extract
    println!("Extracting RetroArch...");
    let file = fs::File::open(&dest_path).map_err(|e| e.to_string())?;
    zip_extract::extract(file, &extract_path, true).map_err(|e| e.to_string())?;

    // 3. Cleanup
    let _ = fs::remove_file(dest_path);

    let exe_path = extract_path.join("retroarch.exe").to_string_lossy().to_string();
    
    // 4. Register in DB
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query(
        "INSERT OR REPLACE INTO emulators (id, name, executable_path) VALUES (?, ?, ?)"
    )
    .bind("retroarch")
    .bind("RetroArch (Auto-Installed)")
    .bind(&exe_path)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(exe_path)
}

#[tauri::command]
pub async fn set_data_root(
    app_handle: tauri::AppHandle,
    path: String,
) -> Result<(), String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let mut config = crate::config::load_config(&app_dir).await.map_err(|e| e.to_string())?;
    
    config.data_root = Some(path);
    crate::config::save_config(app_handle, config).await?;
    
    // The app should probably relaunch now to reload DB from new path
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Emulator {
    pub id: String,
    pub name: String,
    pub executable_path: String,
    pub default_cmdline: Option<String>,
}

#[tauri::command]
pub async fn get_emulators(app_handle: tauri::AppHandle) -> Result<Vec<Emulator>, String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query_as::<_, Emulator>("SELECT * FROM emulators")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_emulator(app_handle: tauri::AppHandle, emulator: Emulator) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query(
        "INSERT OR REPLACE INTO emulators (id, name, executable_path, default_cmdline) VALUES (?, ?, ?, ?)"
    )
    .bind(&emulator.id)
    .bind(&emulator.name)
    .bind(&emulator.executable_path)
    .bind(&emulator.default_cmdline)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_emulator(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query("DELETE FROM emulators WHERE id = ?")
        .bind(&id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn link_platform_emulator(
    app_handle: tauri::AppHandle,
    platform_id: String,
    emulator_id: String,
    is_default: bool,
) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    if is_default {
        sqlx::query("UPDATE platform_emulators SET is_default = 0 WHERE platform_id = ?")
            .bind(&platform_id)
            .execute(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    sqlx::query(
        "INSERT OR REPLACE INTO platform_emulators (platform_id, emulator_id, is_default) VALUES (?, ?, ?)"
    )
    .bind(&platform_id)
    .bind(&emulator_id)
    .bind(is_default as i32)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_platform_emulators(app_handle: tauri::AppHandle, platform_id: String) -> Result<Vec<Emulator>, String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query_as::<_, Emulator>(
        "SELECT e.* FROM emulators e JOIN platform_emulators pe ON e.id = pe.emulator_id WHERE pe.platform_id = ?"
    )
    .bind(&platform_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())
}

