use sqlx::{SqlitePool};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tauri::Manager;
use std::fs;
use std::path::Path;

// Represents a Platform in RAM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Platform {
    pub id: String,
    pub name: String,
    pub category: String,
    pub sort_title: Option<String>,
    pub emulator_id: Option<String>,
    pub folder_path: String,
    pub media_root: Option<String>, // Individual media root
    #[sqlx(skip)]
    pub games: Vec<Game>,
}

// Represents a Game in RAM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Game {
    pub id: String,
    pub platform_id: String,
    pub title: String,
    pub sort_title: Option<String>,
    pub file_path: String,
    pub file_exists: bool,
    pub release_date: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub play_mode: Option<String>,
    pub max_players: Option<u32>,
    pub description: Option<String>,
    pub rating: Option<String>,
    pub region: Option<String>,
    pub play_count: u32,
    pub play_time: u32,
    pub last_played: Option<String>,
    pub completed: bool,
    pub favorite: bool,
    pub star_rating: Option<f32>,
    pub video_path: Option<String>,
    pub scraped: bool,
    #[sqlx(skip)]
    pub images: Vec<Image>, // Images for this game
}

// Represents an Image in RAM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Image {
    pub id: i64,
    pub game_id: String,
    pub image_type: String,
    pub source_path: String,
    pub cache_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

// The main in-memory library state
#[derive(Debug, Default)]
pub struct Library {
    pub platforms: HashMap<String, Platform>,
    pub games: HashMap<String, Game>,
    pub images: HashMap<i64, Image>,
}

impl Library {
    pub async fn load_from_db(pool: &SqlitePool) -> Result<Self> {
        let mut library = Library::default();

        // Load Platforms
        let db_platforms: Vec<Platform> = sqlx::query_as(
            "SELECT id, name, category, sort_title, emulator_id, folder_path, media_root FROM platforms"
        )
        .fetch_all(pool)
        .await?;

        for platform in db_platforms {
            library.platforms.insert(platform.id.clone(), platform);
        }

        // Load Games
        let db_games: Vec<Game> = sqlx::query_as(
            r#"
            SELECT
                id, platform_id, title, sort_title, file_path,
                file_exists, release_date, developer, publisher, genre,
                play_mode, max_players, description, rating, region,
                play_count, play_time, last_played, completed,
                favorite, star_rating, video_path,
                scraped
            FROM games
            "#
        )
        .fetch_all(pool)
        .await?;

        for game in db_games {
            library.games.insert(game.id.clone(), game);
        }

        // Load Images
        let db_images: Vec<Image> = sqlx::query_as(
            "SELECT id, game_id, image_type, source_path, cache_path, width, height FROM images"
        )
        .fetch_all(pool)
        .await?;

        for image in db_images {
            library.images.insert(image.id, image);
        }

        // Link Games to Platforms and Images to Games (optional, can be done on demand)
        // For simplicity, let's keep them separate for now and link on demand.

        Ok(library)
    }
}

// Tauri commands to interact with the library state
#[tauri::command] // Added tauri::command
pub async fn load_library(app_handle: tauri::AppHandle) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    let library_mutex_state = app_handle.state::<tokio::sync::Mutex<Library>>(); // Get State directly
    let mut library_state = library_mutex_state.lock().await;

    match Library::load_from_db(&pool).await {
        Ok(library) => {
            *library_state = library;
            println!("Library loaded into RAM successfully.");
            Ok(())
        },
        Err(e) => {
            eprintln!("Failed to load library from DB: {:?}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command] // Added tauri::command
pub async fn get_games_for_platform(app_handle: tauri::AppHandle, platform_id: String) -> Result<Vec<Game>, String> {
    let library_mutex_state = app_handle.state::<tokio::sync::Mutex<Library>>(); // Get State directly
    let library_state = library_mutex_state.lock().await;
    let games: Vec<Game> = library_state.games.values()
        .filter(|game| game.platform_id == platform_id)
        .cloned()
        .collect();
    Ok(games)
}

#[tauri::command] // Added tauri::command
pub async fn get_platforms(app_handle: tauri::AppHandle) -> Result<Vec<Platform>, String> {
    let library_mutex_state = app_handle.state::<tokio::sync::Mutex<Library>>(); // Get State directly
    let library_state = library_mutex_state.lock().await;
    let platforms: Vec<Platform> = library_state.platforms.values().cloned().collect();
    Ok(platforms)
}

#[tauri::command]
pub async fn add_game(
    app_handle: tauri::AppHandle,
    id: String,
    platform_id: String,
    title: String,
    file_path: String,
) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    sqlx::query(
        "INSERT INTO games (id, platform_id, title, file_path) VALUES (?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&platform_id)
    .bind(&title)
    .bind(&file_path)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    // Refresh memory
    load_library(app_handle).await?;
    
    Ok(())
}

use std::process::Command;
use crate::settings;

#[tauri::command]
pub async fn launch_game(
    app_handle: tauri::AppHandle,
    game_id: String,
) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    // 1. Get Game
    let game: Game = sqlx::query_as("SELECT * FROM games WHERE id = ?")
        .bind(&game_id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Get Platform's default emulator
    let emulator: Option<settings::Emulator> = sqlx::query_as(
        "SELECT e.* FROM emulators e JOIN platform_emulators pe ON e.id = pe.emulator_id WHERE pe.platform_id = ? AND pe.is_default = 1"
    )
    .bind(&game.platform_id)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some(emu) = emulator {
        let mut cmd = Command::new(&emu.executable_path);
        
        if let Some(args) = emu.default_cmdline {
            for arg in args.split_whitespace() {
                cmd.arg(arg);
            }
        }
        
        cmd.arg(&game.file_path);
        
        println!("Launching game: {} with {}", game.title, emu.name);
        cmd.spawn().map_err(|e| format!("Failed to start emulator: {}", e))?;
        Ok(())
    } else {
        Err(format!("No default emulator set for platform {}", game.platform_id))
    }
}

#[tauri::command] // Added tauri::command
pub async fn get_game_images(app_handle: tauri::AppHandle, game_id: String) -> Result<Vec<Image>, String> {
    let library_mutex_state = app_handle.state::<tokio::sync::Mutex<Library>>();
    let library_state = library_mutex_state.lock().await;
    let images: Vec<Image> = library_state.images.values()
        .filter(|image| image.game_id == game_id)
        .cloned()
        .collect();
    Ok(images)
}

#[tauri::command]
pub async fn delete_platform(app_handle: tauri::AppHandle, platform_id: String) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    // 1. Delete games for this platform
    sqlx::query("DELETE FROM games WHERE platform_id = ?")
        .bind(&platform_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Delete platform emulators links
    sqlx::query("DELETE FROM platform_emulators WHERE platform_id = ?")
        .bind(&platform_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Delete platform
    sqlx::query("DELETE FROM platforms WHERE id = ?")
        .bind(&platform_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    // Refresh memory
    load_library(app_handle).await?;
    
    Ok(())
}

#[tauri::command]
pub async fn reset_game_stats(app_handle: tauri::AppHandle, game_id: String) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query("UPDATE games SET play_count = 0, play_time = 0, last_played = NULL WHERE id = ?")
        .bind(&game_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let _ = load_library(app_handle).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_game(app_handle: tauri::AppHandle, game_id: String) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    sqlx::query("DELETE FROM images WHERE game_id = ?")
        .bind(&game_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM games WHERE id = ?")
        .bind(&game_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let _ = load_library(app_handle).await;
    Ok(())
}

#[tauri::command]
pub async fn generate_m3u(game_id: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    let pool = app_handle.state::<SqlitePool>();
    let game: Game = sqlx::query_as("SELECT * FROM games WHERE id = ?").bind(&game_id).fetch_one(&*pool).await.map_err(|e: sqlx::Error| e.to_string())?;
    
    let game_path = Path::new(&game.file_path);
    let parent_dir = game_path.parent().ok_or("Game has no parent directory")?;
    
    // Find all discs (Disc 1, Disc 2, etc)
    let mut discs = Vec::new();
    let entries = fs::read_dir(parent_dir).map_err(|e: std::io::Error| e.to_string())?;
    
    let extensions = ["cue", "bin", "chd", "iso"];
    for entry in entries {
        if let Ok(entry) = entry {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e: &std::ffi::OsStr| e.to_str()) {
                    if extensions.contains(&ext.to_lowercase().as_str()) {
                        // Check if it belongs to this game (primitive check)
                        let fname = p.file_name().unwrap().to_string_lossy();
                        if fname.contains(&game.title) {
                            discs.push(fname.to_string());
                        }
                    }
                }
            }
        }
    }
    
    discs.sort();
    if discs.is_empty() { return Err("No discs found for this game".to_string()); }
    
    let m3u_path = parent_dir.join(format!("{}.m3u", game.title));
    let content = discs.join("\n");
    fs::write(&m3u_path, content).map_err(|e: std::io::Error| e.to_string())?;
    
    Ok(m3u_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn update_game_metadata(
    app_handle: tauri::AppHandle,
    game_id: String,
    data: crate::scraper::ScrapedGameData,
) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    
    sqlx::query(
        r#"
        UPDATE games SET
            developer = ?,
            publisher = ?,
            release_date = ?,
            genre = ?,
            description = ?,
            rating = ?,
            region = ?,
            play_mode = ?,
            max_players = ?,
            star_rating = ?,
            scraped = 1
        WHERE id = ?
        "#
    )
    .bind(data.developer)
    .bind(data.publisher)
    .bind(data.release_date)
    .bind(data.genres)
    .bind(data.description)
    .bind(data.rating)
    .bind(data.region)
    .bind(data.play_mode)
    .bind(data.max_players)
    .bind(data.star_rating)
    .bind(&game_id)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    load_library(app_handle).await?;
    Ok(())
}

#[tauri::command]
pub async fn get_game_versions(title: String, app_handle: tauri::AppHandle) -> Result<Vec<Game>, String> {
    let library_mutex_state = app_handle.state::<tokio::sync::Mutex<Library>>();
    let library_state = library_mutex_state.lock().await;
    
    // Simple version detection: titles that contain this title but are different IDs
    let versions: Vec<Game> = library_state.games.values()
        .filter(|g| g.title.contains(&title) || title.contains(&g.title))
        .cloned()
        .collect();
        
    Ok(versions)
}

#[tauri::command]
pub async fn set_favorite(app_handle: tauri::AppHandle, game_id: String, favorite: bool) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query("UPDATE games SET favorite = ? WHERE id = ?")
        .bind(favorite as i32)
        .bind(&game_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let _ = load_library(app_handle).await;
    Ok(())
}

#[tauri::command]
pub async fn set_star_rating(app_handle: tauri::AppHandle, game_id: String, rating: f32) -> Result<(), String> {
    let pool = app_handle.state::<SqlitePool>();
    sqlx::query("UPDATE games SET star_rating = ? WHERE id = ?")
        .bind(rating)
        .bind(&game_id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;
    
    let _ = load_library(app_handle).await;
    Ok(())
}

#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), String> {
    let path_obj = Path::new(&path);
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        if path_obj.is_file() {
            Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .spawn()
                .map_err(|e: std::io::Error| e.to_string())?;
        } else {
            Command::new("explorer")
                .arg(path)
                .spawn()
                .map_err(|e: std::io::Error| e.to_string())?;
        }
    }
    
    Ok(())
}
