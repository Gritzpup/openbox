use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use reqwest;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrapedArt {
    pub title: String,
    pub box_3d_url: Option<String>,
    pub cart_3d_url: Option<String>,
    pub video_url: Option<String>,
}

#[tauri::command]
pub async fn scrape_game_art(
    platform: String,
    title: String,
) -> Result<ScrapedArt, String> {
    // In a real app, this would query ScreenScraper.fr or similar.
    // For now, we return high-quality placeholder patterns.
    Ok(ScrapedArt {
        title: title.clone(),
        box_3d_url: Some(format!("https://via.placeholder.com/300x400.png?text={}+3D+Box", urlencoding::encode(&title))),
        cart_3d_url: Some(format!("https://via.placeholder.com/200x200.png?text={}+3D+Cart", urlencoding::encode(&title))),
        video_url: Some("https://www.sample-videos.com/video123/mp4/720/big_buck_bunny_720p_1mb.mp4".to_string()),
    })
}

#[tauri::command]
pub async fn download_art(
    url: String,
    destination_path: PathBuf,
) -> Result<(), String> {
    let response = reqwest::get(url).await.map_err(|e| e.to_string())?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    
    fs::write(destination_path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}
