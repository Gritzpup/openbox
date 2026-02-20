use std::path::{PathBuf};
use std::fs;
use image::imageops::FilterType;

#[tauri::command]
pub async fn generate_thumbnail(
    source_path: PathBuf,
    game_id: String,
    cache_dir: PathBuf,
    width: u32,
    height: u32,
) -> Result<PathBuf, String> {
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Failed to create cache directory: {}", e))?;

    let cache_file_name = format!("{}.webp", game_id);
    let cache_path = cache_dir.join(&cache_file_name);

    // If thumbnail exists, return it
    if cache_path.exists() {
        return Ok(cache_path);
    }

    // Load source image
    if !source_path.exists() {
        return Err(format!("Source image not found: {:?}", source_path));
    }

    let img = image::open(&source_path)
        .map_err(|e| format!("Failed to open image: {}", e))?;

    // Resize image maintaining aspect ratio
    let thumbnail = img.resize(width, height, FilterType::Lanczos3);

    // Save as WebP
    thumbnail.save(&cache_path)
        .map_err(|e| format!("Failed to save thumbnail: {}", e))?;

    Ok(cache_path)
}
