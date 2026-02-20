#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod config;
pub mod db;
pub mod scanner;
pub mod media_cache;
pub mod library;
pub mod settings;
pub mod scraper;

use tauri::{Manager};
use std::fs;
use tauri::http::{Response};
use tauri::{UriSchemeContext, Wry};
use std::path::PathBuf;

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
    if let Some(path) = nas_path {
        let log_file = std::path::PathBuf::from(path).join("turbolaunch_telemetry.log");
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let log_line = format!("[{}] {}\n", timestamp, message);
        let _ = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .map(|mut f| {
                use std::io::Write;
                let _ = f.write_all(log_line.as_bytes());
            });
    }
    println!("Log: {}", message);
}

#[tauri::command]
async fn log_to_nas(message: String, nas_path: Option<String>) {
    internal_log_to_nas(message, nas_path).await;
}

#[tauri::command]
async fn get_build_status() -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let response = client.get("https://api.github.com/repos/Gritzpup/openbox/actions/runs?per_page=1")
        .header("User-Agent", "TurboLaunch-App")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    let data: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    
    if let Some(runs) = data["workflow_runs"].as_array() {
        if let Some(last_run) = runs.first() {
            let run_id = last_run["id"].as_i64().unwrap_or(0);
            
            // Check jobs for this specific run to see if any have failed yet
            let jobs_url = format!("https://api.github.com/repos/Gritzpup/openbox/actions/runs/{}/jobs", run_id);
            let jobs_resp = client.get(&jobs_url)
                .header("User-Agent", "TurboLaunch-App")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            
            let jobs_data: serde_json::Value = jobs_resp.json().await.map_err(|e| e.to_string())?;
            let mut any_job_failed = false;
            if let Some(jobs) = jobs_data["jobs"].as_array() {
                for job in jobs {
                    if job["conclusion"] == "failure" {
                        any_job_failed = true;
                        break;
                    }
                }
            }

            return Ok(serde_json::json!({
                "status": last_run["status"],
                "conclusion": if any_job_failed { "failure" } else { last_run["conclusion"].as_str().unwrap_or("") },
                "version": last_run["head_branch"]
            }));
        }
    }
    
    Err("No build runs found".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
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
                        let app_dir = handle.path().app_local_data_dir().expect("Failed to get app local data dir");
                        let _ = internal_log_to_nas(format!("Startup: AppData dir is {:?}", app_dir), None).await;

                        // 1. Load config to find the database directory
                        let config = match config::load_config(&app_dir).await {
                            Ok(c) => {
                                let _ = internal_log_to_nas(format!("Config loaded. Data root: {:?}", c.data_root), c.data_root.clone()).await;
                                c
                            },
                            Err(e) => {
                                let _ = internal_log_to_nas(format!("Config load FAILED: {}", e), None).await;
                                config::AppConfig::default()
                            },
                        };
        
                        let db_dir = if let Some(ref root) = config.data_root {
                            std::path::PathBuf::from(root)
                        } else {
                            app_dir.clone()
                        };
        
                                        // 2. Init DB in the selected directory
                                        match db::init_db(&db_dir).await {
                                            Ok(pool) => {
                                                handle.manage(pool);
                                                
                                                // Auto-scaffold folders if data_root is set
                                                if config.data_root.is_some() {
                                                    let _ = settings::setup_emulator_environment(handle.clone(), db_dir.to_string_lossy().to_string()).await;
                                                }
                        
                                                let pool_state = handle.state::<sqlx::SqlitePool>();                                match library::Library::load_from_db(&pool_state).await {
                                    Ok(lib) => {
                                        let state = handle.state::<tokio::sync::Mutex<library::Library>>();
                                        let mut library_state = state.lock().await;
                                        *library_state = lib;
                                    },
                                    Err(e) => eprintln!("Failed to load library: {}", e),
                                }
                            }
                            Err(e) => {
                                let _ = internal_log_to_nas(format!("DB Init FAILED in {:?}: {}", db_dir, e), config.data_root.clone()).await;
                                eprintln!("Failed to init DB: {}", e);
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
            log_to_nas, report_version, get_build_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
