use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::fs;
use std::path::Path;

pub async fn init_db(app_dir: &Path) -> Result<SqlitePool, sqlx::Error> {
    let db_path = app_dir.join("library.db");
    if !db_path.exists() {
        fs::File::create(&db_path).expect("Failed to create database file");
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_path.to_string_lossy())
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
            sort_title  TEXT,
            emulator_id TEXT,
            folder_path TEXT
        );
        "
    )
    .execute(&pool)
    .await?;

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
