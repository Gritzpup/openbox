use std::path::{Path, PathBuf};
use anyhow::Result;
use sqlx::{SqlitePool};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use std::fs;
use jwalk::WalkDir;
use tauri::Manager;

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "Game")]
pub struct GameXmlEntry {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Platform")]
    pub platform: String,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "SortTitle")]
    pub sort_title: Option<String>,
    #[serde(rename = "Developer")]
    pub developer: Option<String>,
    #[serde(rename = "Publisher")]
    pub publisher: Option<String>,
    #[serde(rename = "Genre")]
    pub genre: Option<String>,
    pub play_mode: Option<String>,
    #[serde(rename = "MaxPlayers")]
    pub max_players: Option<u32>,
    #[serde(rename = "ReleaseDate")]
    pub release_date: Option<String>,
    pub region: Option<String>,
    pub rating: Option<String>,
    #[serde(rename = "Notes")]
    pub description: Option<String>,
    #[serde(rename = "VideoPath")]
    pub video_path: Option<String>,
    #[serde(rename = "FilePath")]
    pub file_path: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename = "LaunchBox")]
pub struct LaunchBoxPlatform {
    #[serde(rename = "Game")]
    pub games: Vec<GameXmlEntry>,
}

pub async fn scan_platform_xml(
    platform_xml_path: &Path,
    _launchbox_root: &Path,
    db: &SqlitePool,
) -> Result<()> {
    let xml_content = fs::read_to_string(platform_xml_path)?;
    let platform_data: LaunchBoxPlatform = from_str(&xml_content)?;

    for game_entry in platform_data.games {
        sqlx::query(
            "
            INSERT OR IGNORE INTO platforms (id, name, folder_path)
            VALUES (?, ?, ?);
            "
        )
        .bind(&game_entry.platform)
        .bind(&game_entry.platform)
        .bind("")
        .execute(db)
        .await?;

        sqlx::query(
            "
            INSERT OR REPLACE INTO games (
                id, platform_id, title, sort_title, file_path,
                release_date, developer, publisher, genre, play_mode,
                max_players, description, rating, region, video_path
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
            "
        )
        .bind(&game_entry.id)
        .bind(&game_entry.platform)
        .bind(&game_entry.title)
        .bind(&game_entry.sort_title)
        .bind(&game_entry.file_path)
        .bind(&game_entry.release_date)
        .bind(&game_entry.developer)
        .bind(&game_entry.publisher)
        .bind(&game_entry.genre)
        .bind(&game_entry.play_mode)
        .bind(game_entry.max_players.map(|p| p as i64))
        .bind(&game_entry.description)
        .bind(&game_entry.rating)
        .bind(&game_entry.region)
        .bind(&game_entry.video_path)
        .execute(db)
        .await?;
    }

    Ok(())
}

fn find_platform_xml_files(launchbox_root: &Path) -> Vec<PathBuf> {
    let mut xml_files = Vec::new();
    let platforms_dir = launchbox_root.join("Data").join("Platforms");

    if !platforms_dir.exists() {
        eprintln!("Launchbox platforms directory not found: {:?}", platforms_dir);
        return xml_files;
    }

    for entry in WalkDir::new(&platforms_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "xml") {
            xml_files.push(path.to_path_buf());
        }
    }
    xml_files
}

#[tauri::command]
pub async fn detect_launchbox() -> Result<Option<String>, String> {
    let mut paths_to_check = vec![
        // Linux Paths
        "/home/ubuntubox/freenas/Emulation/Aaron Program Files (x86)/LaunchBox".to_string(),
        "/home/ubuntubox/freenas/Emulation/Josh Program Files (x86)/LaunchBox".to_string(),
        "/home/ubuntubox/mock_launchbox".to_string(),
        // Windows Paths (UNC)
        "\\\\192.168.1.4\\OurShare\\Emulation\\Josh Program Files (x86)\\LaunchBox".to_string(),
        "\\\\freenas.local\\OurShare\\Emulation\\Josh Program Files (x86)\\LaunchBox".to_string(),
        // Windows Mapped Drives
        "Z:\\Emulation\\Josh Program Files (x86)\\LaunchBox".to_string(),
    ];

    // Check /media/ubuntubox for any external drives that might have LaunchBox
    if let Ok(entries) = fs::read_dir("/media/ubuntubox") {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let lb_path = entry.path().join("LaunchBox");
                    if lb_path.exists() {
                        paths_to_check.push(lb_path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    for path_str in paths_to_check {
        let path = Path::new(&path_str);
        // A valid LaunchBox dir must have Data/Platforms and Metadata
        if path.join("Data").join("Platforms").exists() && path.join("Metadata").exists() {
            println!("Auto-detected LaunchBox at: {}", path_str);
            return Ok(Some(path_str));
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn batch_import(
    app_handle: tauri::AppHandle,
    folder_path: String,
    platform_id: String,
) -> Result<Vec<String>, String> {
    let path = Path::new(&folder_path);
    if !path.exists() || !path.is_dir() {
        return Err("Invalid folder path".to_string());
    }

    let mut games_found = Vec::new();
    let extensions = ["zip", "nes", "smc", "sfc", "iso", "bin", "cue", "gba", "gbc", "n64"];

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    let title = p.file_stem().unwrap().to_string_lossy().to_string();
                    let id = format!("{}-{}", platform_id, title).replace(" ", "-").to_lowercase();
                    
                    // Add to DB
                    let pool = app_handle.state::<SqlitePool>();
                    sqlx::query(
                        "INSERT OR IGNORE INTO games (id, platform_id, title, file_path) VALUES (?, ?, ?, ?)"
                    )
                    .bind(&id)
                    .bind(&platform_id)
                    .bind(&title)
                    .bind(p.to_string_lossy().to_string())
                    .execute(&*pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    games_found.push(title);
                }
            }
        }
    }

    // Reload library
    crate::library::load_library(app_handle).await?;

    Ok(games_found)
}

#[tauri::command]
pub async fn start_scan(app_handle: tauri::AppHandle, launchbox_root: String) -> Result<(), String> {
    let launchbox_path = PathBuf::from(&launchbox_root);
    let db_pool = app_handle.state::<SqlitePool>();

    println!("Starting scan from Launchbox root: {:?}", launchbox_path);

    let xml_files = find_platform_xml_files(&launchbox_path);
    println!("Found {} platform XML files.", xml_files.len());

    for xml_file in xml_files {
        println!("Scanning platform XML: {:?}", xml_file);
        match scan_platform_xml(&xml_file, &launchbox_path, &db_pool).await {
            Ok(_) => println!("Successfully scanned {:?}", xml_file),
            Err(e) => eprintln!("Error scanning {:?}: {}", xml_file, e),
        }
    }

    Ok(())
}
