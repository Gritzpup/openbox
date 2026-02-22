#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod config;
pub mod db;
pub mod scanner;
pub mod media_cache;
pub mod library;
pub mod settings;
pub mod scraper;
pub mod redis_cache;

use tauri::{Manager};
use std::fs;
use tauri::http::{Response};
use tauri::{UriSchemeContext, Wry};
use std::path::PathBuf;

fn setup_panic_hook(nas_path: Option<String>) {
    std::panic::set_hook(Box::new(move |info| {
        let payload = info.payload().downcast_ref::<&str>().unwrap_or(&"unknown panic");
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown location".to_string());
        let msg = format!("CRITICAL PANIC at {}: {}", location, payload);
        
        // 1. Log to local emergency file (existing log)
        if let Some(mut p) = dirs::cache_dir() {
            p.push("TurboLaunch");
            p.push("emergency.log");
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                use std::io::Write;
                let _ = writeln!(f, "[{}] {}", chrono::Local::now(), msg);
            }
        }

        // 2. Log to NAS if available (existing log)
        if let Some(ref path) = nas_path {
            let log_file = std::path::PathBuf::from(path).join("turbolaunch_telemetry.log");
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log_file) {
                use std::io::Write;
                let _ = writeln!(f, "[{}] {}", chrono::Local::now(), msg);
            }
        }
    }));
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[tauri::command]
async fn report_version(version: String, nas_path: Option<String>, error: Option<String>) {
    if let Some(path) = nas_path {
        let version_file = std::path::PathBuf::from(&path).join("active_version.json");
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        
        // Load existing to preserve 'last_updated'
        let mut data = if version_file.exists() {
            let existing = fs::read_to_string(&version_file).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&existing).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        let old_version = data["version"].as_str().unwrap_or("");
        if old_version != "" && old_version != version {
            data["last_updated"] = serde_json::Value::String(timestamp.clone());
        }

        data["version"] = serde_json::Value::String(version);
        data["last_seen"] = serde_json::Value::String(timestamp);
        data["os"] = serde_json::Value::String(std::env::consts::OS.to_string());
        
        if let Some(err) = error {
            data["last_error"] = serde_json::Value::String(err);
        }

        let _ = fs::write(version_file, serde_json::to_string_pretty(&data).unwrap_or_default());
    }
}

pub async fn internal_log_to_nas(message: String, nas_path: Option<String>) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let log_line = format!("[{}] {}\n", timestamp, &message);

    // 1. Try to log to NAS if path is provided (Using Spawn Blocking for safety)
    if let Some(path) = nas_path {
        let msg_clone = log_line.clone();
        tokio::task::spawn_blocking(move || {
            let log_file = std::path::PathBuf::from(&path).join("turbolaunch_telemetry.log");
            if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_file) {
                use std::io::Write;
                let _ = f.write_all(msg_clone.as_bytes());
            }
        });
    }

    // 2. Always log to local cache
    if let Some(mut emergency_path) = dirs::cache_dir() {
        emergency_path.push("TurboLaunch");
        let _ = fs::create_dir_all(&emergency_path);
        emergency_path.push("emergency.log");
        let msg_clone = log_line.clone();
        tokio::task::spawn_blocking(move || {
            if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(emergency_path) {
                use std::io::Write;
                let _ = f.write_all(msg_clone.as_bytes());
            }
        });
    }

    // 3. PUSH TO CENTRAL LOG SERVER (Tilt) - Fire and forget
    let msg_clone = message.clone();
    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
            
        let _ = client.post("http://192.168.1.51:3002/log")
            .json(&serde_json::json!({
                "level": "INFO",
                "source": "RUST-BACKEND",
                "message": msg_clone
            }))
            .send()
            .await;
    });

    println!("Log: {}", message);
}

#[tauri::command]
async fn log_to_nas(message: String, nas_path: Option<String>) {
    internal_log_to_nas(message, nas_path).await;
}

#[tauri::command]
async fn get_build_status() -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let response = client.get("http://192.168.1.51:3001/build-status.json")
        .header("User-Agent", "TurboLaunch-App")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    return Ok(data);
}

#[tauri::command]
async fn test_network() -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
        
    let resp = client.get("http://192.168.1.51:3001/latest.json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
        
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    
    Ok(format!("Status: {}, Length: {}", status, text.len()))
}

#[derive(serde::Deserialize, serde::Serialize)]
struct UpdateInfo {
    version: String,
    notes: String,
    url: String,
}

#[tauri::command]
async fn manual_check_update(current_version: String, handle: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    let app_config = config::get_config(handle).await.unwrap_or_default();
    let nas_path = app_config.data_root.clone();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
        
    let resp = client.get("http://192.168.1.51:3001/latest.json")
        .send()
        .await
        .map_err(|e| e.to_string())?;
        
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    
    let remote_version = json["version"].as_str().ok_or("Missing version in JSON")?;
    let notes = json["notes"].as_str().unwrap_or("").to_string();
    let url = json["platforms"]["windows-x86_64"]["url"].as_str()
        .ok_or("Missing windows-x86_64 url in JSON")?.to_string();

    let remote_clean = remote_version.trim_start_matches('v').trim();
    let current_clean = current_version.trim_start_matches('v').trim();

    let _ = internal_log_to_nas(
        format!("Update check: current='{}' (clean='{}'), remote='{}' (clean='{}')", current_version, current_clean, remote_version, remote_clean),
        nas_path
    ).await;

    if remote_clean != current_clean {
        Ok(Some(UpdateInfo { version: remote_version.to_string(), notes, url }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn manual_install_update(url: String, app_handle: tauri::AppHandle) -> Result<(), String> {
    let client = reqwest::Client::builder().build().map_err(|e| e.to_string())?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;

    let temp_dir = std::env::temp_dir();
    let installer_path = temp_dir.join("TurboLaunch_Update_Installer.exe");
    fs::write(&installer_path, bytes).map_err(|e| e.to_string())?;

    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let current_exe_str = current_exe.to_string_lossy().to_string();
    let installer_str = installer_path.to_string_lossy().to_string();
    
    let app_config = config::get_config(app_handle.clone()).await.unwrap_or_default();
    let nas_path = app_config.data_root.clone();
    let log_path = if let Some(ref p) = nas_path {
        std::path::PathBuf::from(p).join("turbolaunch_telemetry.log").to_string_lossy().to_string()
    } else {
        "NUL".to_string()
    };
    
    let _ = internal_log_to_nas(format!("Updating to {}... Exiting app. Current exe: {}", url, current_exe_str), nas_path.clone()).await;

    // Generate a temporary batch file for the update process
    let bat_path = temp_dir.join("turbolaunch_updater.bat");
    let bat_content = format!(
        "@echo off\n\
         echo [%DATE% %TIME%] [UPDATE] Starting update script... >> \"{log}\"\n\
         timeout /t 5 /nobreak > NUL\n\
         echo [%DATE% %TIME%] [UPDATE] Killing processes and WebView2... >> \"{log}\"\n\
         taskkill /F /IM turbolaunch.exe /T >> \"{log}\" 2>&1\n\
         taskkill /F /IM TurboLaunch.exe /T >> \"{log}\" 2>&1\n\
         taskkill /F /IM TurboLaunch_*.exe /T >> \"{log}\" 2>&1\n\
         taskkill /F /IM msedgewebview2.exe /T >> \"{log}\" 2>&1\n\
         timeout /t 5 /nobreak > NUL\n\
         echo [%DATE% %TIME%] [UPDATE] Running installer: {inst} >> \"{log}\"\n\
         start /wait \"\" \"{inst}\" /S\n\
         echo [%DATE% %TIME%] [UPDATE] Installer finished with code %ERRORLEVEL% >> \"{log}\"\n\
         timeout /t 10 /nobreak > NUL\n\
         echo [%DATE% %TIME%] [UPDATE] Relaunching: {exe} >> \"{log}\"\n\
         start \"\" \"{exe}\"\n\
         echo [%DATE% %TIME%] [UPDATE] Script complete. >> \"{log}\"\n\
         del \"%~f0\" & exit\n",
        log = log_path,
        inst = installer_str,
        exe = current_exe_str
    );
    
    fs::write(&bat_path, bat_content).map_err(|e| e.to_string())?;

    // Launch the batch file fully detached
    std::process::Command::new("cmd")
        .args(["/C", "start", "/B", "", &bat_path.to_string_lossy()])
        .spawn()
        .map_err(|e| format!("Failed to launch update script: {}", e))?;

    app_handle.exit(0);
    Ok(())
}

#[tauri::command]
fn file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::async_runtime::spawn(async move {
        let current_exe = std::env::current_exe().unwrap_or_default();
        let config_dir = dirs::data_local_dir().unwrap_or_default().join("com.turbolaunch.app");
        let config = crate::config::load_config(&config_dir).await.unwrap_or_default();
        let _ = internal_log_to_nas(format!("🚀 APP STARTUP: v0.1.159 | Path: {:?}", current_exe), config.data_root).await;
    });

    let _ = tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        // .plugin(tauri_plugin_updater::Builder::new().build())
        .register_uri_scheme_protocol("game-media", move |_context: UriSchemeContext<Wry>, request| {
            let uri = request.uri().to_string();
            let path_str = uri.replace("game-media://localhost", "");
            
            let decoded_path = urlencoding::decode(&path_str).unwrap_or(std::borrow::Cow::Borrowed(&path_str));
            let path = PathBuf::from(decoded_path.to_string());

            if path.exists() && path.is_file() {
                match fs::read(&path) {
                    Ok(data) => {
                        let mime_type = if path.extension().map_or(false, |ext| ext == "webp") {
                            "image/webp"
                        } else if path.extension().map_or(false, |ext| ext == "png") {
                            "image/png"
                        } else if path.extension().map_or(false, |ext| ext == "jpg" || ext == "jpeg") {
                            "image/jpeg"
                        } else {
                            "application/octet-stream"
                        };

                        Response::builder()
                            .status(200)
                            .header("Access-Control-Allow-Origin", "*")
                            .header("Content-Type", mime_type)
                            .body(data)
                            .unwrap()
                    }
                    Err(_) => Response::builder().status(500).body(vec![]).unwrap(),
                }
            } else {
                Response::builder().status(404).body(vec![]).unwrap()
            }
        })
                .setup(|app| {
                    app.manage(tokio::sync::Mutex::new(library::Library::default()));
        
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let app_dir = match handle.path().app_local_data_dir() {
                            Ok(dir) => {
                                setup_panic_hook(None);
                                dir
                            },
                            Err(e) => {
                                setup_panic_hook(None);
                                let _ = internal_log_to_nas(format!("CRITICAL: Failed to get AppData dir: {}", e), None).await;
                                return;
                            }
                        };
                        let _ = internal_log_to_nas(format!("Startup: AppData dir is {:?}", app_dir), None).await;
                        let _ = internal_log_to_nas("Window state plugin initialized.".to_string(), None).await;

                        // 1. Load config to find the database directory
                        let config = match config::load_config(&app_dir).await {
                            Ok(c) => {
                                setup_panic_hook(c.data_root.clone());
                                let _ = internal_log_to_nas(format!("Config loaded successfully. Data root: {:?}", c.data_root), c.data_root.clone()).await;
                                c
                            },
                            Err(e) => {
                                let _ = internal_log_to_nas(format!("Config load FAILED: {}. Using default.", e), None).await;
                                config::AppConfig::default()
                            },
                        };
        
                        let db_dir = if let Some(ref root) = config.data_root {
                            std::path::PathBuf::from(root)
                        } else {
                            app_dir.clone()
                        };
                        
                        let _ = internal_log_to_nas(format!("Attempting to init DB in {:?}", db_dir), config.data_root.clone()).await;
        
                        // 2. Init DB in the selected directory
                        match db::init_db(&db_dir).await {
                            Ok(pool) => {
                                let _ = internal_log_to_nas("DB initialized successfully. Managing pool...".to_string(), config.data_root.clone()).await;
                                handle.manage(pool);
                                
                                // Auto-scaffold folders if data_root is set
                                if config.data_root.is_some() {
                                    let _ = settings::setup_emulator_environment(handle.clone(), db_dir.to_string_lossy().to_string()).await;
                                }
                        
                                let pool_state = handle.state::<sqlx::SqlitePool>();
                                let _ = internal_log_to_nas("Loading library from DB into RAM...".to_string(), config.data_root.clone()).await;
                                
                                match library::Library::load_from_db(&pool_state).await {
                                    Ok(lib) => {
                                        let _ = internal_log_to_nas(format!("Library loaded! {} platforms, {} games.", lib.platforms.len(), lib.games.len()), config.data_root.clone()).await;
                                        let state = handle.state::<tokio::sync::Mutex<library::Library>>();
                                        let mut library_state = state.lock().await;
                                        *library_state = lib;
                                    },
                                    Err(e) => {
                                        let _ = internal_log_to_nas(format!("Library load from DB FAILED: {}", e), config.data_root.clone()).await;
                                    },
                                }
                            }
                            Err(e) => {
                                let _ = internal_log_to_nas(format!("DB Init FAILED in {:?}: {}", db_dir, e), config.data_root.clone()).await;
                            },
                        }
                    });
                    Ok(())
                })        .invoke_handler(tauri::generate_handler![
            greet, 
            config::get_config, config::save_config, scanner::start_scan, scanner::detect_launchbox, scanner::batch_import,
            library::load_library, library::get_games_for_platform, library::get_platforms, library::get_game_images, library::add_game, library::launch_game, library::delete_platform,
            media_cache::generate_thumbnail,
            settings::get_emulators, settings::save_emulator, settings::delete_emulator,
            settings::link_platform_emulator, settings::get_platform_emulators,
            settings::set_data_root, settings::install_retroarch, settings::install_emulator, settings::setup_emulator_environment, settings::scaffold_platform_directories,
            settings::sync_emulators,
            scraper::scrape_game_art, scraper::download_art,
            log_to_nas, report_version, get_build_status, test_network,
            manual_check_update, manual_install_update, file_exists,
            library::reset_game_stats, library::delete_game, library::open_folder,
            library::generate_m3u, library::get_game_versions, library::update_game_metadata,
            library::set_favorite, library::set_star_rating, library::check_ra_compatibility, scraper::index_metadata
        ])
        .run(tauri::generate_context!());
}
