use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub data_root: Option<String>, // The master folder on the NAS for DB/Settings/Media
    pub media_root: String,
    pub global_media_root: Option<String>,
    pub cache_dir: String,
    pub thumbnail_width: u32,
    pub thumbnail_height: u32,
    pub startup_verify_paths: bool,
    pub theme: String,
    pub ra_username: String,
    pub ra_api_key: String,
    pub last_platform_id: Option<String>,
    pub last_game_id: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            data_root: None,
            media_root: "".to_string(),
            global_media_root: None,
            cache_dir: "".to_string(),
            thumbnail_width: 300,
            thumbnail_height: 400,
            startup_verify_paths: true,
            theme: "dark".to_string(),
            ra_username: "".to_string(),
            ra_api_key: "CpwgrPoMcC9w9PNuq4ZHbVu6pBEJPmJ5".to_string(),
            last_platform_id: None,
            last_game_id: None,
        }
    }
}

pub fn get_config_path(app_dir: &Path) -> PathBuf {
    app_dir.join("config.json")
}

pub async fn load_config(app_dir: &Path) -> Result<AppConfig> {
    let mut data_root: Option<String> = None;

    // 1. Check for Portable Pointer (next to exe)
    // This is the most persistent way to store the NAS path
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let portable_ptr = exe_dir.join("portable_data_root.txt");
            if portable_ptr.exists() {
                if let Ok(path) = fs::read_to_string(&portable_ptr) {
                    let trimmed = path.trim().to_string();
                    if !trimmed.is_empty() {
                        data_root = Some(trimmed);
                    }
                }
            }
        }
    }

    // 2. Check for local pointer config (local_config.json in AppData) if portable wasn't found
    if data_root.is_none() {
        let local_config_path = app_dir.join("local_config.json");
        if local_config_path.exists() {
            if let Ok(content) = fs::read_to_string(&local_config_path) {
                if let Ok(local) = serde_json::from_str::<serde_json::Value>(&content) {
                    data_root = local["data_root"].as_str().map(|s| s.to_string());
                }
            }
        }
    }

    // 3. Determine where the master config should be
    let master_dir = if let Some(ref path) = data_root {
        PathBuf::from(path)
    } else {
        app_dir.to_path_buf()
    };

    let config_path = master_dir.join("config.json");
    
    // 4. Attempt to load existing config
    if config_path.exists() {
        match fs::read_to_string(&config_path) {
            Ok(config_str) => {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&config_str) {
                    config.data_root = data_root;
                    return Ok(config);
                }
            },
            Err(_) => {
                let mut config = AppConfig::default();
                config.data_root = data_root;
                return Ok(config); 
            }
        }
    }

    // 5. No config found
    let mut config = AppConfig::default();
    config.data_root = data_root;
    
    if config.data_root.is_none() {
        let _ = save_config_internal(app_dir, config.clone()).await;
    }
    
    Ok(config)
}

async fn save_config_internal(app_dir: &Path, config: AppConfig) -> Result<()> {
    // 1. Save local pointer (AppData)
    let local_config_path = app_dir.join("local_config.json");
    let local_json = serde_json::json!({ "data_root": config.data_root });
    fs::write(local_config_path, serde_json::to_string_pretty(&local_json)?)?;

    // 2. Save portable pointer (next to exe) if we have a data_root
    if let Some(ref root) = config.data_root {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let portable_ptr = exe_dir.join("portable_data_root.txt");
                let _ = fs::write(portable_ptr, root);
            }
        }
    }

    // 3. Save master config
    let master_dir = if let Some(ref path) = config.data_root {
        PathBuf::from(path)
    } else {
        app_dir.to_path_buf()
    };

    if !master_dir.exists() {
        let _ = fs::create_dir_all(&master_dir);
    }

    let config_path = master_dir.join("config.json");
    let config_str = serde_json::to_string_pretty(&config)?;
    let _ = fs::write(config_path, config_str);
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
