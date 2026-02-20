use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub launchbox_root: String,
    pub cache_dir: String,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub startup_verify_paths: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            launchbox_root: "".to_string(),
            cache_dir: "".to_string(),
            thumbnail_width: 300,
            thumbnail_height: 400,
            startup_verify_paths: true,
            theme: "dark".to_string(),
        }
    }
}

pub fn get_config_path(app_dir: &Path) -> PathBuf {
    app_dir.join("config.json")
}

pub async fn load_config(app_dir: &Path) -> Result<AppConfig> {
    let config_path = get_config_path(app_dir);
    if config_path.exists() {
        let config_str = fs::read_to_string(config_path)?;
        let config: AppConfig = serde_json::from_str(&config_str)?;
        Ok(config)
    } else {
        let config = AppConfig::default();
        save_config_internal(app_dir, config.clone()).await?; // Use internal save
        Ok(config)
    }
}

async fn save_config_internal(app_dir: &Path, config: AppConfig) -> Result<()> {
    let config_path = get_config_path(app_dir);
    let config_str = serde_json::to_string_pretty(&config)?;
    fs::write(config_path, config_str)?;
    Ok(())
}

// Tauri Commands

#[tauri::command]
pub async fn get_config(app_handle: AppHandle) -> Result<AppConfig, String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    load_config(&app_dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_config(app_handle: AppHandle, config: AppConfig) -> Result<(), String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    save_config_internal(&app_dir, config).await.map_err(|e| e.to_string())
}
