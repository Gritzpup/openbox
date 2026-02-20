use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use reqwest;
use std::fs;

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

#[tauri::command]
pub async fn scrape_game_art(
    platform: String,
    title: String,
) -> Result<ScrapedArt, String> {
    // In a production app, this queries ScreenScraper.fr or LaunchBox DB.
    // For now, we simulate high-quality results for all requested types.
    let safe_title = urlencoding::encode(&title);
    
    Ok(ScrapedArt {
        title: title.clone(),
        box_3d_url: Some(format!("https://via.placeholder.com/300x400.png?text={}+3D+Box", safe_title)),
        box_front_url: Some(format!("https://via.placeholder.com/300x400.png?text={}+Front+Box", safe_title)),
        box_back_url: Some(format!("https://via.placeholder.com/300x400.png?text={}+Back+Box", safe_title)),
        box_full_url: None,
        box_front_reconstructed_url: None,
        box_back_reconstructed_url: None,
        flyer_front_url: Some(format!("https://via.placeholder.com/300x500.png?text={}+Flyer+Front", safe_title)),
        flyer_back_url: None,
        arcade_cabinet_url: None,
        arcade_marquee_url: None,
        arcade_board_url: None,
        arcade_control_panel_url: None,
        arcade_controls_info_url: None,
        banner_url: None,
        clear_logo_url: Some(format!("https://via.placeholder.com/400x200.png?text={}+Logo", safe_title)),
        fanart_background_url: Some(format!("https://via.placeholder.com/1920x1080.png?text={}+Fanart", safe_title)),
        disc_url: Some(format!("https://via.placeholder.com/300x300.png?text={}+Disc", safe_title)),
        cart_3d_url: Some(format!("https://via.placeholder.com/200x200.png?text={}+3D+Cart", safe_title)),
        cart_front_url: None,
        cart_back_url: None,
        bigbox_video_url: Some("https://www.sample-videos.com/video123/mp4/720/big_buck_bunny_720p_1mb.mp4".to_string()),
        gameplay_video_url: Some("https://www.sample-videos.com/video123/mp4/720/big_buck_bunny_720p_1mb.mp4".to_string()),
        screenshot_gameplay_url: Some(format!("https://via.placeholder.com/640x480.png?text={}+Gameplay", safe_title)),
        screenshot_title_url: Some(format!("https://via.placeholder.com/640x480.png?text={}+Title", safe_title)),
        screenshot_select_url: None,
        screenshot_gameover_url: None,
        screenshot_scores_url: None,
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
