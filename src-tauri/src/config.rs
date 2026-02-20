use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub data_root: Option<String>, // The master folder on the NAS for DB/Settings/Media
    pub launchbox_root: String,
    pub media_root: String,
    pub global_media_root: Option<String>,
    pub cache_dir: String,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub startup_verify_paths: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            data_root: None,
            launchbox_root: "".to_string(),
            media_root: "".to_string(),
            global_media_root: None,
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
    // 1. Check for local pointer config
    let local_config_path = app_dir.join("local_config.json");
    let mut data_root: Option<String> = None;

    if local_config_path.exists() {
        let content = fs::read_to_string(&local_config_path)?;
        let local: serde_json::Value = serde_json::from_str(&content)?;
        data_root = local["data_root"].as_str().map(|s| s.to_string());
    }

    // 2. Determine where the master config is
    let master_dir = if let Some(ref path) = data_root {
        PathBuf::from(path)
    } else {
        app_dir.to_path_buf()
    };

    if !master_dir.exists() {
        fs::create_dir_all(&master_dir)?;
    }

    let config_path = master_dir.join("config.json");
    if config_path.exists() {
        let config_str = fs::read_to_string(config_path)?;
        let mut config: AppConfig = serde_json::from_str(&config_str)?;
        config.data_root = data_root;
        Ok(config)
    } else {
        let mut config = AppConfig::default();
        config.data_root = data_root;
        save_config_internal(app_dir, config.clone()).await?;
        Ok(config)
    }
}

async fn save_config_internal(app_dir: &Path, config: AppConfig) -> Result<()> {
    // 1. Save local pointer
    let local_config_path = app_dir.join("local_config.json");
    let local_json = serde_json::json!({ "data_root": config.data_root });
    fs::write(local_config_path, serde_json::to_string_pretty(&local_json)?)?;

    // 2. Save master config
    let master_dir = if let Some(ref path) = config.data_root {
        PathBuf::from(path)
    } else {
        app_dir.to_path_buf()
    };

    if !master_dir.exists() {
        fs::create_dir_all(&master_dir)?;
    }

    let config_path = master_dir.join("config.json");
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
