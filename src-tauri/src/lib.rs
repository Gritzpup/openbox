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
async fn log_to_nas(message: String, nas_path: Option<String>) {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
                        
                        // 1. Load config to find the database directory
                        let config = match config::load_config(&app_dir).await {
                            Ok(c) => c,
                            Err(_) => config::AppConfig::default(),
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
                                let pool_state = handle.state::<sqlx::SqlitePool>();
                                match library::Library::load_from_db(&pool_state).await {
                                    Ok(lib) => {
                                        let state = handle.state::<tokio::sync::Mutex<library::Library>>();
                                        let mut library_state = state.lock().await;
                                        *library_state = lib;
                                    },
                                    Err(e) => eprintln!("Failed to load library: {}", e),
                                }
                            }
                            Err(e) => eprintln!("Failed to init DB: {}", e),
                        }
                    });
                    Ok(())
                })        .invoke_handler(tauri::generate_handler![
            greet, 
            config::get_config, config::save_config, scanner::start_scan, scanner::detect_launchbox, scanner::batch_import,
            library::load_library, library::get_games_for_platform, library::get_platforms, library::get_game_images, library::add_game, library::launch_game,
            media_cache::generate_thumbnail,
            settings::get_emulators, settings::save_emulator, settings::delete_emulator,
            settings::link_platform_emulator, settings::get_platform_emulators,
            settings::set_data_root, settings::install_retroarch,
            scraper::scrape_game_art, scraper::download_art,
            log_to_nas
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
