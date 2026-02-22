# OpenBox (TurboLaunch) Project Overview

## Core Concept
OpenBox is a high-performance, cross-platform game library manager and launcher inspired by LaunchBox. Its primary differentiator is a **NAS-centric architecture**, designed to allow multiple machines (Windows or Linux) to share a single, synchronized source of truth for games, metadata, media, and emulator configurations.

## Architecture

### 1. Technology Stack
- **Frontend:** Svelte 5 / SvelteKit (UI/UX)
- **Backend:** Tauri (Rust) for system-level operations (File I/O, SQLite, Process execution)
- **Database:** SQLite (SQLx) for structured data.
- **Communication:** Tauri IPC (Invoke) for Frontend-to-Backend calls.

### 2. Storage Strategy (The "Master NAS Folder")
Unlike traditional launchers that store data in `%APPDATA%`, OpenBox relies on a user-defined **Master Data Root** (typically a NAS share like `//192.168.1.4/OurShare/OpenBox`).
This folder contains:
- `library.db`: The global SQLite database containing all game and platform records.
- `config.json`: Master application settings.
- `Images/`: Categorized game art (3D Boxes, Fronts, Logos, etc.).
- `Videos/`: Cinematic trailers and gameplay clips.
- `Emulators/`: Portable emulator installations (RetroArch, PCSX2, etc.) that can be run directly from the NAS.
- `Games/`: (Optional) The actual ROM files if the user chooses to "Move" or "Copy" games during import.

### 3. Cross-Platform Path Handling
- **Windows:** Typically accesses the NAS via mapped drives (e.g., `Z:\Emulation\...`).
- **Linux:** Accesses the NAS via mount points (e.g., `/home/ubuntubox/freenas/...`).
- The application logic includes path translation and "portable pointers" (`portable_data_root.txt`) to ensure instances can find the Master Folder regardless of the host OS.

## Key Features

### 1. Import Games Wizard
A comprehensive 5-step workflow modeled after LaunchBox:
- **Platform Selection:** Select from standard gaming categories (Consoles, Handhelds, etc.).
- **Emulator Linking:** Assign an emulator to the platform.
- **File Management:** 
    - *Link:* Keep files in their current location (best for existing NAS collections).
    - *Copy:* Duplicate games into the NAS `Games/` folder.
    - *Move:* Physically relocate files to the NAS.
- **Metadata & Media Overhaul:** Select specific art types to scrape (3D Boxes, Flyers, Marquees, Gameplay videos).
- **Batch Processing:** Scans directories, identifies games, and registers them in the library.

### 2. Emulator Auto-Setup
Automatically downloads, extracts, and configures portable versions of popular emulators directly onto the NAS:
- RetroArch
- PCSX2
- RPCS3
- xemu (Xbox)

### 3. Library Management
- **Platform Sidebar:** Organized by category (Consoles, Arcade, etc.).
- **Game Grid:** Fast, thumbnail-based browsing with support for localized image caching.
- **Launch Logic:** Seamlessly executes emulators with the correct command-line arguments for the selected game.
- **Standalone Media & Metadata:** 
    - **Master Metadata Database:** OpenBox is now fully standalone. It uses a master `Metadata.xml` file stored in the NAS `Data/` folder to index game information. 
    - **NAS-Native Media:** Art and videos are stored directly on the NAS in an organized folder structure (e.g., `NAS/Images/Platform/Type/`).
    - **Filename Sanitization:** Implements robust filename matching (e.g., replacing `:` with `_`) to ensure games with special characters are correctly matched to their art.
    - **RetroAchievements Integration:** Automatically verifies game compatibility with RetroAchievements by hashing files (including internal ZIP content) and displaying a 🏆 trophy icon in the UI.
    - **Independent Scraping (Planned):** Future versions will include built-in web scraping (e.g., ScreenScraper or GamesDB) to download art directly from the internet.

### 4. Synchronization & Telemetry
- **Telemetry Logging:** Logs are written back to the NAS (`turbolaunch_telemetry.log`) so errors on any machine can be diagnosed from a central location. This includes `[SCRAPE]` and `[MEDIA]` events for troubleshooting download failures.
- **Auto-Updater:** Integrated plugin to keep all app instances on the latest version via GitHub releases.

## Current Maintenance State
- **Standalone Transition (v0.1.143):** Removed all hard dependencies on local LaunchBox installations. The app now scaffolds its own directory structure and manages its own master metadata database on the NAS.
- **RetroAchievements Integration:** Added automatic hashing (including ZIP support) and 🏆 trophy icons for compatible games.
- **Persistence & Selection:** Fixed issues where the app wouldn't remember the last selected game and resolved update loops caused by version mismatches.
- **Database Schema:** Supports categorized platforms and exhaustive media types. Includes migration logic to handle schema updates across shared instances.
