use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use reqwest;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct ScrapedArt {
    pub title: String,
    pub box_3d_url: Option<String>,
}

#[tauri::command]
pub async fn scrape_game_art(
    platform: String,
    title: String,
) -> Result<ScrapedArt, String> {
    // This is a placeholder. Real implementation would search a DB like ScreenScraper or TGDB.
    // For now, we return a mock URL to demonstrate the flow.
    // In a real scenario, you'd perform an HTTP GET to a search API.
    
    Ok(ScrapedArt {
        title: title.clone(),
        box_3d_url: Some(format!("https://via.placeholder.com/300x400.png?text={}+3D+Box", urlencoding::encode(&title))),
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
