use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, sqlite::SqliteConnectOptions};
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub async fn init_db(db_dir: &Path) -> Result<SqlitePool, sqlx::Error> {
    if !db_dir.exists() {
        if let Err(e) = fs::create_dir_all(db_dir) {
            eprintln!("Failed to create database directory: {}", e);
            return Err(sqlx::Error::Io(e));
        }
    }
    let db_path = db_dir.join("library.db");
    
    let connect_options = SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.to_string_lossy()))?
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(10));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect_with(connect_options)
        .await?;

    sqlx::query(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-64000;
        PRAGMA temp_store=MEMORY;
        "
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS platforms (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            category    TEXT DEFAULT 'Consoles',
            sort_title  TEXT,
            emulator_id TEXT,
            folder_path TEXT,
            media_root  TEXT
        );
        "
    )
    .execute(&pool)
    .await?;

    // Migration: Add category column if it doesn't exist
    let _ = sqlx::query("ALTER TABLE platforms ADD COLUMN category TEXT DEFAULT 'Consoles';")
        .execute(&pool)
        .await;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS games (
            id               TEXT PRIMARY KEY,
            platform_id      TEXT NOT NULL REFERENCES platforms(id),
            title            TEXT NOT NULL,
            sort_title       TEXT,
            file_path        TEXT NOT NULL,
            file_exists      INTEGER DEFAULT 1,
            release_date     TEXT,
            developer        TEXT,
            publisher        TEXT,
            genre            TEXT,
            play_mode        TEXT,
            max_players      INTEGER,
            description      TEXT,
            rating           TEXT,
            region           TEXT,
            play_count       INTEGER DEFAULT 0,
            play_time        INTEGER DEFAULT 0,
            last_played      TEXT,
            completed        INTEGER DEFAULT 0,
            favorite         INTEGER DEFAULT 0,
            star_rating      REAL,
            video_path       TEXT,
            scraped          INTEGER DEFAULT 0
        );
        "
    )
    .execute(&pool)
    .await?;

    // --- MIGRATIONS ---
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN release_date TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN developer TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN publisher TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN genre TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN play_mode TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN max_players INTEGER;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN description TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN rating TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN region TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN play_time INTEGER DEFAULT 0;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN favorite INTEGER DEFAULT 0;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN star_rating REAL;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN video_path TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN scraped INTEGER DEFAULT 0;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN file_hash TEXT;").execute(&pool).await;
    let _ = sqlx::query("ALTER TABLE games ADD COLUMN ra_game_id INTEGER;").execute(&pool).await;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS metadata (
            id               TEXT PRIMARY KEY,
            title            TEXT NOT NULL,
            search_title     TEXT,
            platform         TEXT,
            release_date     TEXT,
            developer        TEXT,
            publisher        TEXT,
            genres           TEXT,
            max_players      INTEGER,
            description      TEXT,
            rating           TEXT,
            star_rating      REAL
        );
        "
    )
    .execute(&pool)
    .await?;

    let _ = sqlx::query("ALTER TABLE metadata ADD COLUMN search_title TEXT;").execute(&pool).await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_metadata_title ON metadata(title);")
        .execute(&pool)
        .await?;
    
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_metadata_search ON metadata(search_title, platform);")
        .execute(&pool)
        .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS images (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            game_id     TEXT NOT NULL REFERENCES games(id),
            image_type  TEXT NOT NULL,
            source_path TEXT NOT NULL,
            cache_path  TEXT,
            width       INTEGER,
            height      INTEGER
        );
        "
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS emulators (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL,
            executable_path TEXT NOT NULL,
            default_cmdline TEXT
        );
        "
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS platform_emulators (
            platform_id  TEXT REFERENCES platforms(id),
            emulator_id  TEXT REFERENCES emulators(id),
            is_default   INTEGER DEFAULT 0,
            cmdline_override TEXT,
            PRIMARY KEY (platform_id, emulator_id)
        );
        "
    )
    .execute(&pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_games_platform ON games(platform_id);")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_games_title ON games(sort_title);")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_images_game ON images(game_id);")
        .execute(&pool)
        .await?;

    Ok(pool)
}
