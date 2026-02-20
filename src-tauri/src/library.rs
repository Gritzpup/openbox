use sqlx::{SqlitePool};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tauri::Manager; // Added Manager import

// Represents a Platform in RAM
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Platform {
    pub id: String,
    pub name: String,
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
            "SELECT id, name, sort_title, emulator_id, folder_path, media_root FROM platforms"
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
                play_count, last_played, completed,
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
