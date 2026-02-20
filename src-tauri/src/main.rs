#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod db;
mod scanner;
mod media_cache;
mod library;
mod settings;

use tauri::{Manager};
use std::fs;
use tauri::http::{Response};
use tauri::{UriSchemeContext, Wry};
use std::path::PathBuf;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("game-media", move |_context: UriSchemeContext<Wry>, request| {
            // The URI will look like game-media://localhost/home/ubuntubox/...
            // We need to strip the scheme and host to get the absolute path
            let uri = request.uri().to_string();
            let path_str = uri.replace("game-media://localhost", "");
            
            // Decode percent-encoded characters (like spaces)
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
            // Initialize the in-memory library state
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
                        println!("Database initialized successfully.");

                        // Load the library from DB into RAM after DB init
                        let pool_state = handle.state::<sqlx::SqlitePool>();
                        let library_mutex_state = handle.state::<tokio::sync::Mutex<library::Library>>();
                        let mut library_state = library_mutex_state.lock().await;
                        match library::Library::load_from_db(&pool_state).await {
                            Ok(lib) => {
                                *library_state = lib;
                                println!("Initial library loaded into RAM.");
                            },
                            Err(e) => eprintln!("Failed to load initial library into RAM: {}", e),
                        }
                    }
                    Err(e) => {
                        println!("Failed to initialize database: {}", e);
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet, 
            config::get_config, config::save_config, scanner::start_scan, scanner::detect_launchbox,
            library::load_library, library::get_games_for_platform, library::get_platforms, library::get_game_images, library::add_game,
            media_cache::generate_thumbnail,
            settings::get_emulators, settings::save_emulator, settings::delete_emulator
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
