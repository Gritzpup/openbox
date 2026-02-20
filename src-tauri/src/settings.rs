use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use tauri::Manager;

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
