use std::path::{PathBuf};
use serde::{Deserialize, Serialize};
use reqwest;
use std::fs;
use tauri::{Manager, Emitter};
use sqlx::{SqlitePool, Row};
use quick_xml::reader::Reader;
use quick_xml::events::Event;
use crate::redis_cache::RedisCache;
use jwalk::WalkDir;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedArt {
    pub title: String,
    pub box_3d_url: Option<String>,
    pub box_front_url: Option<String>,
    pub box_back_url: Option<String>,
    pub box_full_url: Option<String>,
    pub box_front_reconstructed_url: Option<String>,
    pub box_back_reconstructed_url: Option<String>,
    pub flyer_front_url: Option<String>,
    pub flyer_back_url: Option<String>,
    pub arcade_cabinet_url: Option<String>,
    pub arcade_marquee_url: Option<String>,
    pub arcade_board_url: Option<String>,
    pub arcade_control_panel_url: Option<String>,
    pub arcade_controls_info_url: Option<String>,
    pub banner_url: Option<String>,
    pub clear_logo_url: Option<String>,
    pub fanart_background_url: Option<String>,
    pub disc_url: Option<String>,
    pub cart_3d_url: Option<String>,
    pub cart_front_url: Option<String>,
    pub cart_back_url: Option<String>,
    pub bigbox_video_url: Option<String>,
    pub gameplay_video_url: Option<String>,
    pub screenshot_gameplay_url: Option<String>,
    pub screenshot_title_url: Option<String>,
    pub screenshot_select_url: Option<String>,
    pub screenshot_gameover_url: Option<String>,
    pub screenshot_scores_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScrapedGameData {
    pub title: String,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    pub genres: Option<String>,
    pub description: Option<String>,
    pub rating: Option<String>,
    pub region: Option<String>,
    pub play_mode: Option<String>,
    pub max_players: Option<u32>,
    pub star_rating: Option<f32>,
    pub art: ScrapedArt,
}

#[tauri::command]
pub async fn index_metadata(app_handle: tauri::AppHandle) -> Result<String, String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let config = crate::config::load_config(&app_dir).await.map_err(|e| e.to_string())?;
    
    let xml_path = if let Some(nas) = &config.data_root {
        PathBuf::from(nas).join("Data").join("Metadata.xml")
    } else {
        return Err("NAS Data Root not configured".to_string());
    };

    if !xml_path.exists() { 
        return Err(format!("Metadata.xml not found at: {:?}", xml_path)); 
    }

    let pool = app_handle.state::<SqlitePool>();
    
    // Attempt to get a connection with a retry policy for busy DB
    let mut tx = pool.begin().await.map_err(|e| format!("Failed to start transaction (is DB locked?): {}", e))?;

    sqlx::query("DELETE FROM metadata").execute(&mut *tx).await.map_err(|e| e.to_string())?;

    let mut reader = Reader::from_file(&xml_path).map_err(|e| e.to_string())?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut count = 0;
    
    let mut current_id = String::new();
    let mut current_name = String::new();
    let mut current_platform = String::new();
    let mut current_release_date = String::new();
    let mut current_developer = String::new();
    let mut current_publisher = String::new();
    let mut current_genres = String::new();
    let mut current_max_players = String::new();
    let mut current_overview = String::new();
    let mut current_rating = String::new();
    let mut current_star_rating = String::new();
    
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().map_err(|e| e.to_string())?.to_string();
                match current_tag.as_str() {
                    "DatabaseID" => current_id = text,
                    "Name" => current_name = text,
                    "Platform" => current_platform = text,
                    "ReleaseDate" | "ReleaseYear" => current_release_date = text,
                    "Developer" => current_developer = text,
                    "Publisher" => current_publisher = text,
                    "Genres" => current_genres = text,
                    "MaxPlayers" => current_max_players = text,
                    "Overview" => current_overview = text,
                    "ESRB" => current_rating = text,
                    "CommunityRating" => current_star_rating = text,
                    _ => ()
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let name = String::from_utf8_lossy(qname.as_ref());
                if name == "Game" {
                    if !current_id.is_empty() {
                        let search_title = normalize_title(&current_name);
                        sqlx::query(
                            "INSERT OR IGNORE INTO metadata (id, title, search_title, platform, release_date, developer, publisher, genres, max_players, description, rating, star_rating) 
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                        )
                        .bind(&current_id)
                        .bind(&current_name)
                        .bind(&search_title)
                        .bind(&current_platform)
                        .bind(&current_release_date)
                        .bind(&current_developer)
                        .bind(&current_publisher)
                        .bind(&current_genres)
                        .bind(current_max_players.parse::<i32>().unwrap_or(1))
                        .bind(&current_overview)
                        .bind(&current_rating)
                        .bind(current_star_rating.parse::<f32>().unwrap_or(0.0))
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        
                        count += 1;
                        if count % 500 == 0 {
                            let _ = app_handle.emit("index-progress", serde_json::json!({ "count": count, "percent": 0 }));
                        }
                    }
                    current_id.clear(); current_name.clear(); current_platform.clear(); 
                    current_release_date.clear(); current_developer.clear(); 
                    current_publisher.clear(); current_genres.clear(); 
                    current_max_players.clear(); current_overview.clear(); 
                    current_rating.clear(); current_star_rating.clear();
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("Error at position {}: {:?}", reader.buffer_position(), e)),
            _ => (),
        }
        buf.clear();
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    let msg = format!("Successfully indexed {} games from Metadata.xml", count);
    let _ = crate::internal_log_to_nas(msg.clone(), config.data_root).await;
    Ok(msg)
}

fn sanitize_filename(title: &str) -> String {
    let invalid_chars = [':', '?', '*', '|', '<', '>', '"', '/', '\\'];
    let mut sanitized = title.to_string();
    for c in invalid_chars {
        sanitized = sanitized.replace(c, "_");
    }
    sanitized = sanitized.replace("---", " - ");
    sanitized = sanitized.replace("--", " - ");
    sanitized
}

fn normalize_title(title: &str) -> String {
    let mut normalized = title.to_lowercase();
    if let Some(pos) = normalized.find(" (") { normalized.truncate(pos); }
    if let Some(pos) = normalized.find(" [") { normalized.truncate(pos); }
    normalized = normalized.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn find_nas_media(nas_root: &str, platform: &str, title: &str, folder: &str) -> Option<String> {
    if nas_root.is_empty() { return None; }
    
    let platforms_to_try = vec![
        platform.to_string(),
        platform.replace("PlayStation", "Playstation"),
        platform.replace("Playstation", "PlayStation"),
        platform.replace("Sony ", ""),
    ];

    let sanitized = sanitize_filename(title);
    let mut titles_to_try = vec![title.to_string(), sanitized.clone()];
    
    if let Some(pos) = title.find(" (") {
        let simpler = &title[..pos];
        titles_to_try.push(simpler.to_string());
        titles_to_try.push(sanitize_filename(simpler));
    }

    for p_variant in platforms_to_try {
        let base_path = PathBuf::from(nas_root).join("Images").join(&p_variant).join(folder);
        if !base_path.exists() { continue; }

        let _ = crate::internal_log_to_nas(format!("[MEDIA-DEBUG] Searching recursively in {:?}", base_path), Some(nas_root.to_string())).await;

        // Recursive search to handle region subfolders like "North America"
        for entry in WalkDir::new(&base_path).max_depth(3) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    let file_name = entry.file_name().to_string_lossy().to_lowercase();
                    for t in &titles_to_try {
                        let t_lower = t.to_lowercase();
                        if file_name.starts_with(&t_lower) {
                            let path_str = entry.path().to_string_lossy().to_string();
                            let _ = crate::internal_log_to_nas(format!("[MEDIA-DEBUG] FOUND MATCH: {}", path_str), Some(nas_root.to_string())).await;
                            return Some(path_str);
                        }
                    }
                }
            }
        }
    }
    let _ = crate::internal_log_to_nas(format!("[MEDIA-DEBUG] No match found for '{}' in folder '{}'", title, folder), Some(nas_root.to_string())).await;
    None
}

async fn find_nas_video(nas_root: &str, platform: &str, title: &str) -> Option<String> {
    if nas_root.is_empty() { return None; }
    
    let platforms_to_try = vec![
        platform.to_string(),
        platform.replace("PlayStation", "Playstation"),
        platform.replace("Playstation", "PlayStation"),
        platform.replace("Sony ", ""),
    ];

    let sanitized = sanitize_filename(title);
    let mut titles_to_try = vec![title.to_string(), sanitized.clone()];
    
    if let Some(pos) = title.find(" (") {
        let simpler = &title[..pos];
        titles_to_try.push(simpler.to_string());
    }

    for p_variant in platforms_to_try {
        let base_path = PathBuf::from(nas_root).join("Videos").join(&p_variant);
        if !base_path.exists() { continue; }

        for entry in WalkDir::new(&base_path).max_depth(3) {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    let file_name = entry.file_name().to_string_lossy().to_lowercase();
                    for t in &titles_to_try {
                        let t_lower = t.to_lowercase();
                        if file_name.starts_with(&t_lower) {
                            return Some(entry.path().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

#[tauri::command]
pub async fn scrape_game_art(
    app_handle: tauri::AppHandle,
    platform: String,
    title: String,
) -> Result<ScrapedGameData, String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let config = crate::config::load_config(&app_dir).await.map_err(|e| e.to_string())?;
    let pool = app_handle.state::<SqlitePool>();
    let nas_path = config.data_root.clone();
    
    let cache = RedisCache::new();
    let cache_key = format!("metadata:{}:{}", platform, title);
    if let Some(cached_data) = cache.get::<ScrapedGameData>(&cache_key).await {
        return Ok(cached_data);
    }

    let platforms_to_try = vec![
        platform.clone(),
        platform.replace("PlayStation", "Playstation"),
        platform.replace("Playstation", "PlayStation"),
        "Sony PlayStation 2".to_string(),
        "Sony Playstation 2".to_string(),
        "Sony PlayStation".to_string(),
        "Sony Playstation".to_string(),
    ];

    let mut db_metadata: Option<sqlx::sqlite::SqliteRow> = None;
    let normalized = normalize_title(&title);

    for p_variant in &platforms_to_try {
        db_metadata = sqlx::query("SELECT * FROM metadata WHERE title = ? AND platform = ? LIMIT 1")
            .bind(&title)
            .bind(p_variant)
            .fetch_optional(&*pool)
            .await
            .map_err(|e| e.to_string())?;
        
        if db_metadata.is_some() { break; }

        db_metadata = sqlx::query("SELECT * FROM metadata WHERE search_title = ? AND platform = ? LIMIT 1")
            .bind(&normalized)
            .bind(p_variant)
            .fetch_optional(&*pool)
            .await
            .map_err(|e| e.to_string())?;
        
        if db_metadata.is_some() { break; }

        db_metadata = sqlx::query("SELECT * FROM metadata WHERE search_title LIKE ? AND platform = ? LIMIT 1")
            .bind(format!("{}%", normalized))
            .bind(p_variant)
            .fetch_optional(&*pool)
            .await
            .map_err(|e| e.to_string())?;
        
        if db_metadata.is_some() { break; }
    }

    let art_search_title = if let Some(ref row) = db_metadata {
        row.get::<String, _>("title")
    } else {
        title.clone()
    };

    let nas_root_str = nas_path.clone().unwrap_or_default();

    let art = ScrapedArt {
        title: title.clone(),
        box_3d_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - 3D").await,
        box_front_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - Front").await,
        box_back_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - Back").await,
        box_full_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - Full").await,
        box_front_reconstructed_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - Front Reconstructed").await,
        box_back_reconstructed_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Box - Back Reconstructed").await,
        flyer_front_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Advertisement Flyer - Front").await,
        flyer_back_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Advertisement Flyer - Back").await,
        arcade_cabinet_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Arcade - Cabinet").await,
        arcade_marquee_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Arcade - Marquee").await,
        arcade_board_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Arcade - Circuit Board").await,
        arcade_control_panel_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Arcade - Control Panel").await,
        arcade_controls_info_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Arcade - Controls Info").await,
        banner_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Banner").await,
        clear_logo_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Clear Logo").await,
        fanart_background_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Fanart - Background").await,
        disc_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Disc").await,
        cart_3d_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Cart - 3D").await,
        cart_front_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Cart - Front").await,
        cart_back_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Cart - Back").await,
        bigbox_video_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Video - Big Box Cinematic").await,
        gameplay_video_url: find_nas_video(&nas_root_str, &platform, &art_search_title).await,
        screenshot_gameplay_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Screenshot - Gameplay").await,
        screenshot_title_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Screenshot - Game Title").await,
        screenshot_select_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Screenshot - Game Select").await,
        screenshot_gameover_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Screenshot - Game Over").await,
        screenshot_scores_url: find_nas_media(&nas_root_str, &platform, &art_search_title, "Screenshot - High Scores").await,
    };

    let mut result = ScrapedGameData {
        title: title.clone(),
        developer: None,
        publisher: None,
        release_date: None,
        genres: None,
        description: None,
        rating: None,
        region: None,
        play_mode: None,
        max_players: None,
        star_rating: None,
        art,
    };

    if let Some(row) = db_metadata {
        result.developer = row.get("developer");
        result.publisher = row.get("publisher");
        result.release_date = row.get("release_date");
        result.genres = row.get("genres");
        result.description = row.get("description");
        result.rating = row.get("rating");
        result.star_rating = row.get("star_rating");
    }

    if result.art.box_front_url.is_none() {
        let safe_title = urlencoding::encode(&title);
        result.art.box_front_url = Some(format!("https://via.placeholder.com/300x400.png?text={}", safe_title));
    }

    cache.set(&cache_key, &result, 86400).await;
    Ok(result)
}

#[tauri::command]
pub async fn download_art(
    app_handle: tauri::AppHandle,
    url: String,
    destination_path: PathBuf,
) -> Result<(), String> {
    let app_dir = app_handle.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let config = crate::config::load_config(&app_dir).await.map_err(|e| e.to_string())?;
    let nas_path = config.data_root.clone();

    if destination_path.exists() { 
        let _ = crate::internal_log_to_nas(format!("[MEDIA] Skipping download, already exists: {:?}", destination_path), nas_path).await;
        return Ok(()); 
    }

    if url.starts_with("http") {
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap();
        let response = client.get(url).send().await.map_err(|e| e.to_string())?;
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&destination_path, bytes).map_err(|e| e.to_string())?;
    } else {
        let source_path = PathBuf::from(&url);
        if source_path.exists() {
            let _ = crate::internal_log_to_nas(format!("[MEDIA] Copying local file: {:?} to {:?}", source_path, destination_path), nas_path).await;
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(source_path, destination_path).map_err(|e| e.to_string())?;
        } else {
            let _ = crate::internal_log_to_nas(format!("[MEDIA] ERROR: Source art NOT FOUND at: {:?}", source_path), nas_path).await;
        }
    }
    Ok(())
}
