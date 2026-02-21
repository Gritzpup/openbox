use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use reqwest;
use std::fs;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize)]
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

fn find_local_lb_art(lb_root: &str, platform: &str, title: &str, folder: &str) -> Option<String> {
    if lb_root.is_empty() { return None; }
    
    let base_path = PathBuf::from(lb_root).join("Images").join(platform).join(folder);
    if !base_path.exists() { return None; }

    // Try various extensions
    let extensions = ["png", "jpg", "jpeg", "gif", "webp"];
    for ext in extensions {
        // Try exact match
        let p = base_path.join(format!("{}.{}", title, ext));
        if p.exists() { return Some(p.to_string_lossy().to_string()); }
        
        // Try -01 match (LaunchBox default)
        let p = base_path.join(format!("{}-01.{}", title, ext));
        if p.exists() { return Some(p.to_string_lossy().to_string()); }
    }
    None
}

#[derive(Debug, Serialize, Deserialize)]
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

fn find_local_lb_metadata(lb_root: &str, platform: &str, title: &str) -> Option<serde_json::Value> {
    if lb_root.is_empty() { return None; }
    
    let xml_path = PathBuf::from(lb_root).join("Data").join("Platforms").join(format!("{}.xml", platform));
    if !xml_path.exists() { return None; }

    let content = fs::read_to_string(xml_path).ok()?;
    
    // We do a very primitive search to avoid heavy XML parsing of 100MB files
    // Find the <Game> block containing the title
    if let Some(pos) = content.find(&format!("<Title>{}</Title>", title)) {
        // Look backwards for <Game> and forwards for </Game>
        let start = content[..pos].rfind("<Game>").unwrap_or(0);
        let end = content[pos..].find("</Game>").map(|e| pos + e + 7).unwrap_or(content.len());
        let game_xml = &content[start..end];

        // Helper to extract a tag
        let get_tag = |tag: &str| {
            let open = format!("<{}>", tag);
            let close = format!("</{}>", tag);
            if let Some(s) = game_xml.find(&open) {
                if let Some(e) = game_xml.find(&close) {
                    let val = &game_xml[s + open.len()..e];
                    if !val.trim().is_empty() { return Some(val.to_string()); }
                }
            }
            None
        };

        return Some(serde_json::json!({
            "developer": get_tag("Developer"),
            "publisher": get_tag("Publisher"),
            "release_date": get_tag("ReleaseDate"),
            "genres": get_tag("Genres"),
            "description": get_tag("Notes"),
            "rating": get_tag("Rating"),
            "region": get_tag("Region"),
            "play_mode": get_tag("PlayMode"),
            "max_players": get_tag("MaxPlayers"),
            "star_rating": get_tag("StarRatingFloat"),
        }));
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
    let lb_root = config.launchbox_root;

    // Normalize platform name for LB folders
    let lb_platform = match platform.as_str() {
        "Sony PlayStation 2" => "Sony Playstation 2",
        "Sony PlayStation 3" => "Sony Playstation 3",
        "Sony PlayStation" => "Sony Playstation",
        "Sony PlayStation Portable" => "Sony Playstation Portable",
        _ => &platform
    };

    // 1. Get Metadata from XML
    let metadata = find_local_lb_metadata(&lb_root, lb_platform, &title);

    // 2. Get Art from Folders
    let art = ScrapedArt {
        title: title.clone(),
        box_3d_url: find_local_lb_art(&lb_root, lb_platform, &title, "Box - 3D"),
        box_front_url: find_local_lb_art(&lb_root, lb_platform, &title, "Box - Front"),
        box_back_url: find_local_lb_art(&lb_root, lb_platform, &title, "Box - Back"),
        box_full_url: find_local_lb_art(&lb_root, lb_platform, &title, "Box - Full"),
        box_front_reconstructed_url: None,
        box_back_reconstructed_url: None,
        flyer_front_url: find_local_lb_art(&lb_root, lb_platform, &title, "Advertisement Flyer - Front"),
        flyer_back_url: find_local_lb_art(&lb_root, lb_platform, &title, "Advertisement Flyer - Back"),
        arcade_cabinet_url: find_local_lb_art(&lb_root, lb_platform, &title, "Arcade - Cabinet"),
        arcade_marquee_url: find_local_lb_art(&lb_root, lb_platform, &title, "Arcade - Marquee"),
        arcade_board_url: find_local_lb_art(&lb_root, lb_platform, &title, "Arcade - Circuit Board"),
        arcade_control_panel_url: find_local_lb_art(&lb_root, lb_platform, &title, "Arcade - Control Panel"),
        arcade_controls_info_url: find_local_lb_art(&lb_root, lb_platform, &title, "Arcade - Controls Info"),
        banner_url: find_local_lb_art(&lb_root, lb_platform, &title, "Banner"),
        clear_logo_url: find_local_lb_art(&lb_root, lb_platform, &title, "Clear Logo"),
        fanart_background_url: find_local_lb_art(&lb_root, lb_platform, &title, "Fanart - Background"),
        disc_url: find_local_lb_art(&lb_root, lb_platform, &title, "Disc"),
        cart_3d_url: find_local_lb_art(&lb_root, lb_platform, &title, "Cart - 3D"),
        cart_front_url: find_local_lb_art(&lb_root, lb_platform, &title, "Cart - Front"),
        cart_back_url: find_local_lb_art(&lb_root, lb_platform, &title, "Cart - Back"),
        bigbox_video_url: find_local_lb_art(&lb_root, lb_platform, &title, "Video - Big Box Cinematic"),
        gameplay_video_url: find_local_lb_art(&lb_root, lb_platform, &title, "Video - Gameplay"),
        screenshot_gameplay_url: find_local_lb_art(&lb_root, lb_platform, &title, "Screenshot - Gameplay"),
        screenshot_title_url: find_local_lb_art(&lb_root, lb_platform, &title, "Screenshot - Game Title"),
        screenshot_select_url: find_local_lb_art(&lb_root, lb_platform, &title, "Screenshot - Game Select"),
        screenshot_gameover_url: find_local_lb_art(&lb_root, lb_platform, &title, "Screenshot - Game Over"),
        screenshot_scores_url: find_local_lb_art(&lb_root, lb_platform, &title, "Screenshot - High Scores"),
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

    if let Some(m) = metadata {
        result.developer = m["developer"].as_str().map(|s| s.to_string());
        result.publisher = m["publisher"].as_str().map(|s| s.to_string());
        result.release_date = m["release_date"].as_str().map(|s| s.to_string());
        result.genres = m["genres"].as_str().map(|s| s.to_string());
        result.description = m["description"].as_str().map(|s| s.to_string());
        result.rating = m["rating"].as_str().map(|s| s.to_string());
        result.region = m["region"].as_str().map(|s| s.to_string());
        result.play_mode = m["play_mode"].as_str().map(|s| s.to_string());
        result.max_players = m["max_players"].as_str().and_then(|s| s.parse().ok());
        result.star_rating = m["star_rating"].as_str().and_then(|s| s.parse().ok());
    }

    // Fallback art
    if result.art.box_front_url.is_none() {
        let safe_title = urlencoding::encode(&title);
        result.art.box_front_url = Some(format!("https://via.placeholder.com/300x400.png?text={}", safe_title));
    }

    Ok(result)
}

#[tauri::command]
pub async fn download_art(
    url: String,
    destination_path: PathBuf,
) -> Result<(), String> {
    // If destination exists, skip
    if destination_path.exists() { return Ok(()); }

    if url.starts_with("http") {
        let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        
        fs::write(destination_path, bytes).map_err(|e| e.to_string())?;
    } else {
        // Local file path (from LaunchBox)
        let source_path = PathBuf::from(url);
        if source_path.exists() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(source_path, destination_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
