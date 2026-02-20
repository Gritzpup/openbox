use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use tauri::Manager;

use std::path::PathBuf;
use std::fs;
use zip_extract;

#[tauri::command]
pub async fn scaffold_platform_directories(
    app_handle: tauri::AppHandle,
    master_path: String,
    platform_id: String,
) -> Result<(), String> {
    let base_image_dir = PathBuf::from(&master_path).join("Images").join(&platform_id);
    let base_video_dir = PathBuf::from(&master_path).join("Videos").join(&platform_id);

    let image_subfolders = [
        "Box - 3D", "Box - Front", "Box - Back", "Cart - 3D", "Cart - Front",
        "Clear Logo", "Disc", "Screenshot - Gameplay", "Fanart - Background"
    ];

    for folder in image_subfolders {
        let path = base_image_dir.join(folder);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }
    }

    if !base_video_dir.exists() {
        fs::create_dir_all(&base_video_dir).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn setup_emulator_environment(
    app_handle: tauri::AppHandle,
    master_path: String,
) -> Result<(), String> {
    let master = PathBuf::from(&master_path);
    let folders = ["Emulators", "Images", "Videos", "Cache", "Data"];
    
    for folder in folders {
        let path = master.join(folder);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }
    }

    // Auto-sync emulators on startup
    let _ = sync_emulators(app_handle, master_path).await;
    Ok(())
}

#[tauri::command]
pub async fn sync_emulators(
    app_handle: tauri::AppHandle,
    master_path: String,
) -> Result<(), String> {
    let emu_dir = PathBuf::from(&master_path).join("Emulators");
    let version_file = emu_dir.join("emulator_versions.json");

    let mut versions: serde_json::Value = if version_file.exists() {
        let content = fs::read_to_string(&version_file).unwrap_or_else(|_| "{}".to_string());
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let required = [
        ("retroarch", "1.22.2"),
        ("pcsx2", "2.2.0"),
        ("rpcs3", "0.0.38"),
        ("xemu", "0.8.133")
    ];

    for (id, latest_ver) in required {
        let current_ver = versions[id].as_str().unwrap_or("");
        if current_ver != latest_ver {
            let log_msg = format!("Syncing emulator {}: {} -> {}", id, current_ver, latest_ver);
            println!("{}", log_msg);
            let _ = crate::internal_log_to_nas(log_msg, Some(master_path.clone())).await;
            
            match install_emulator(app_handle.clone(), id.to_string(), master_path.clone()).await {
                Ok(_) => {
                    versions[id] = serde_json::json!(latest_ver);
                },
                Err(e) => {
                    let err_msg = format!("Failed to sync {}: {}", id, e);
                    println!("{}", err_msg);
                    let _ = crate::internal_log_to_nas(err_msg, Some(master_path.clone())).await;
                }
            }
        }
    }

    let _ = fs::write(version_file, serde_json::to_string_pretty(&versions).unwrap_or_default());
    Ok(())
}

#[tauri::command]
pub async fn install_emulator(
    app_handle: tauri::AppHandle,
    emu_id: String,
    master_path: String,
) -> Result<String, String> {
    let emu_dir = PathBuf::from(&master_path).join("Emulators");
    
    let (url, name, zip_name, sub_folder, exe_name) = match emu_id.as_str() {
        "retroarch" => (
            "https://buildbot.libretro.com/stable/1.22.2/windows/x86_64/RetroArch.zip",
            "RetroArch",
            "RetroArch.zip",
            "RetroArch",
            "retroarch.exe"
        ),
        "pcsx2" => (
            "https://github.com/PCSX2/pcsx2/releases/download/v2.2.0/pcsx2-v2.2.0-windows-x64-Qt.zip",
            "PCSX2",
            "pcsx2.zip",
            "PCSX2",
            "pcsx2-qt.exe"
        ),
        "rpcs3" => (
            "https://github.com/RPCS3/rpcs3-binaries-win/releases/download/build-ff9401303b387cb11b97cf5984a9ab7672f487fc/rpcs3_v0.0.38-18441_win64.zip",
            "RPCS3",
            "rpcs3.zip",
            "RPCS3",
            "rpcs3.exe"
        ),
        "xemu" => (
            "https://github.com/xemu-project/xemu/releases/download/v0.8.133/xemu-0.8.133-windows-x86_64.zip",
            "xemu",
            "xemu.zip",
            "xemu",
            "xemu.exe"
        ),
        _ => return Err("Unsupported emulator".to_string()),
    };

    let dest_path = emu_dir.join(zip_name);
    let extract_path = emu_dir.join(sub_folder);

    // 1. Download
    let log_msg = format!("Downloading {}...", name);
    println!("{}", log_msg);
    let _ = crate::internal_log_to_nas(log_msg, Some(master_path.clone())).await;

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    let response = client.get(url).send().await.map_err(|e| format!("Download error: {}", e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Failed to download {}: HTTP {}", name, status));
    }

    let bytes = response.bytes().await.map_err(|e| format!("Byte error: {}", e))?;
    
    if bytes.len() < 1024 * 1024 {
        return Err(format!("Download for {} seems too small ({} bytes). Is the link dead?", name, bytes.len()));
    }

    let log_msg = format!("Download complete ({} MB). Writing to NAS...", bytes.len() / 1024 / 1024);
    let _ = crate::internal_log_to_nas(log_msg, Some(master_path.clone())).await;

    fs::write(&dest_path, bytes).map_err(|e| format!("Write error: {}", e))?;

    // 2. Extract
    let log_msg = format!("Extracting {}...", name);
    println!("{}", log_msg);
    let _ = crate::internal_log_to_nas(log_msg, Some(master_path.clone())).await;

    let file = fs::File::open(&dest_path).map_err(|e| format!("Open error: {}", e))?;
    zip_extract::extract(file, &extract_path, true).map_err(|e| format!("Extraction failed for {}: {}", name, e))?;

    // 3. Cleanup
    let _ = fs::remove_file(dest_path);

    let exe_path = extract_path.join(exe_name).to_string_lossy().to_string();
    
    // 4. Register in DB
    let log_msg = format!("{} installed successfully.", name);
    let _ = crate::internal_log_to_nas(log_msg, Some(master_path.clone())).await;
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query(
        "INSERT OR REPLACE INTO emulators (id, name, executable_path) VALUES (?, ?, ?)"
    )
    .bind(&emu_id)
    .bind(name)
    .bind(&exe_path)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(exe_path)
}

#[tauri::command]
pub async fn install_retroarch(
    app_handle: tauri::AppHandle,
    target_dir: String,
) -> Result<String, String> {
    install_emulator(app_handle, "retroarch".to_string(), target_dir).await
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

