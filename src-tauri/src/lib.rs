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
                if !app_dir.exists() {
                    fs::create_dir_all(&app_dir).expect("Failed to create app local data dir");
                }
                match db::init_db(&app_dir).await {
                    Ok(pool) => {
                        handle.manage(pool);
                        match library::Library::load_from_db(&handle.state::<sqlx::SqlitePool>()).await {
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
        })
        .invoke_handler(tauri::generate_handler![
            greet, 
            config::get_config, config::save_config, scanner::start_scan, scanner::detect_launchbox, scanner::batch_import,
            library::load_library, library::get_games_for_platform, library::get_platforms, library::get_game_images, library::add_game,
            media_cache::generate_thumbnail,
            settings::get_emulators, settings::save_emulator, settings::delete_emulator,
            scraper::scrape_game_art, scraper::download_art
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
