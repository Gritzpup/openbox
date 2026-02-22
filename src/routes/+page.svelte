<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { listen } from "@tauri-apps/api/event";
    import { open } from "@tauri-apps/plugin-dialog";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { check } from '@tauri-apps/plugin-updater';
    import { relaunch } from '@tauri-apps/plugin-process';

    let platforms = $state([]);
    let selectedPlatform = $state(null);
    let games = $state([]);
    let loading = $state(false);
    let config = $state({ data_root: null, global_media_root: "" });
    let currentView = $state("library"); 
    let menuOpen = $state(false);
    let updateStatus = $state("");
    let lastChecked = $state("");
    let isUpdating = $state(false);
    let isChecking = $state(false);
    let isDragging = $state(false);
    let updateError = $state("");
    let logs = $state([]);

    let emulators = $state([]);
    let newEmulator = $state({ id: "", name: "", executable_path: "", default_cmdline: "" });
    let platformEmulators = $state([]);
    let thumbnails = $state({}); 

    let platformMenuOpen = $state(false);

    // Wizard/Setup State
    let setupWizardOpen = $state(false);
    let importWizardOpen = $state(false);
    let emulatorSetupOpen = $state(false);
    let wizardStep = $state(1); 
    let wizardFiles = $state([]);
    let wizardPlatform = $state("");
    let wizardCategory = $state("Consoles");
    let wizardEmulator = $state(null);
    let wizardFileAction = $state("link"); // link, copy, move
    let wizardSearchMetadata = $state(true);
    let wizardMediaOptions = $state({
        box_3d: true,
        box_front: true,
        box_back: true,
        box_full: false,
        box_front_reconstructed: false,
        box_back_reconstructed: false,
        flyer_front: false,
        flyer_back: false,
        arcade_cabinet: false,
        arcade_marquee: false,
        arcade_board: false,
        arcade_control_panel: false,
        arcade_controls_info: false,
        banner: false,
        bigbox_video: true,
        gameplay_video: true,
        clear_logo: true,
        fanart_background: true,
        disc: true,
        cart_3d: true,
        cart_front: true,
        cart_back: true,
        screenshot_gameplay: true,
        screenshot_title: true,
        screenshot_select: false,
        screenshot_gameover: false,
        screenshot_scores: false
    });
    let selectedGames = $state([]);
    let selectedGame = $derived(games.find(g => g.id === selectedGames[selectedGames.length - 1]));
    let favoriteGames = $derived(games.filter(g => g.favorite));
    let otherGames = $derived(games.filter(g => !g.favorite));

    let selectedGameMedia = $state({ screenshots: [], box3d: null, logo: null, video: null });

    async function toggleFavorite(gameId) {
        const game = games.find(g => g.id === gameId);
        if (!game) return;
        const newState = !game.favorite;
        try {
            await invoke("set_favorite", { gameId, favorite: newState });
            game.favorite = newState;
            addLog(`Game ${game.title} ${newState ? 'added to' : 'removed from'} favorites.`);
        } catch (e) { addLog("Failed to toggle favorite: " + e); }
    }

    async function setRating(gameId, rating) {
        try {
            await invoke("set_star_rating", { gameId, rating });
            const game = games.find(g => g.id === gameId);
            if (game) game.star_rating = rating;
        } catch (e) { addLog("Failed to set rating: " + e); }
    }

    async function loadSelectedGameMedia(game) {
        selectedGameMedia = { screenshots: [], box3d: null, logo: null, video: null };
        const platform = platforms.find(p => p.id === game.platform_id);
        const mediaRoot = platform?.media_root || config.global_media_root || config.data_root || "";
        if (!mediaRoot) return;

        try {
            const images = await invoke("get_game_images", { gameId: game.id });
            selectedGameMedia.screenshots = images.filter(img => img.image_type.includes("Screenshot")).map(img => `game-media://localhost${img.cache_path || img.source_path}`);
            
            const box3d = images.find(img => img.image_type === "Box - 3D");
            if (box3d) selectedGameMedia.box3d = `game-media://localhost${box3d.cache_path || box3d.source_path}`;
            
            const logo = images.find(img => img.image_type === "Clear Logo");
            if (logo) selectedGameMedia.logo = `game-media://localhost${logo.cache_path || logo.source_path}`;

            if (game.video_path) {
                selectedGameMedia.video = `game-media://localhost${game.video_path}`;
            }
        } catch (e) { console.error("Failed to load game media", e); }
    }

    $effect(() => {
        if (selectedGame) {
            loadSelectedGameMedia(selectedGame);
        }
    });

    let toolsMenuOpen = $state(false);
    let downloadMenuOpen = $state(false);
    let viewMenuOpen = $state(false);
    let imageGroupMenuOpen = $state(false);
    let activeImageGroup = $state("3D Boxes");

    function sanitizeFilename(title) {
        if (!title) return "";
        // Replace chars: : ? * | < > " / \
        return title.replace(/[:?*|<>"\/\\]/g, "_");
    }

    let globalProgress = $state({ active: false, title: "", detail: "", percent: 0 });

    const IMAGE_GROUPS = {
        "Backgrounds": ["Fanart - Background"],
        "Boxes": ["Box - Front", "Box - 3D"],
        "3D Boxes": ["Box - 3D", "Box - Front"],
        "Cards": ["Cart - Front", "Cart - 3D"],
        "3D Cards": ["Cart - 3D", "Cart - Front"],
        "Clear Logos": ["Clear Logo"],
        "Marquees": ["Arcade - Marquee", "Banner"],
        "Screenshots": ["Screenshot - Gameplay", "Screenshot - Game Title"],
        "Steam Banners": ["Banner"]
    };

    function setImageGroup(group) {
        activeImageGroup = group;
        localStorage.setItem("activeImageGroup", group);
        // Clear thumbnails to force reload
        thumbnails = {};
        games.forEach(g => loadThumbnail(g));
    }

    let multiUpdateWizardOpen = $state(false);
    let multiUpdateStep = $state(1);
    let multiUpdateReplace = $state(false); // Default: do not replace

    function toggleSelection(gameId, event) {
        if (event.ctrlKey || event.shiftKey) {
            if (selectedGames.includes(gameId)) {
                selectedGames = selectedGames.filter(id => id !== gameId);
            } else {
                selectedGames = [...selectedGames, gameId];
            }
        } else {
            selectedGames = [gameId];
        }
        saveLastState();
    }

    let clearSelection = () => { selectedGames = []; };

    let contextMenu = $state({ open: false, x: 0, y: 0, gameId: null });
    let contextMenuVersions = $state([]);
    let playlists = $state([]);

    async function openContextMenu(e, gameId) {
        e.preventDefault();
        e.stopPropagation();
        contextMenu = { open: true, x: e.clientX, y: e.clientY, gameId };
        
        // Fetch versions
        const game = games.find(g => g.id === gameId);
        if (game) {
            try {
                contextMenuVersions = await invoke("get_game_versions", { title: game.title });
            } catch (err) { contextMenuVersions = []; }

            // Auto-trigger RA check if not already known
            if (!game.ra_game_id) {
                invoke("check_ra_compatibility", { gameId }).then(async (newId) => {
                    if (newId) {
                        // Refresh local game list or just update the one reference
                        game.ra_game_id = newId;
                        // Refresh versions too
                        try {
                            contextMenuVersions = await invoke("get_game_versions", { title: game.title });
                        } catch (err) {}
                    }
                });
            }
        }
    }

    function closeContextMenu() {
        contextMenu.open = false;
    }

    async function loadPlaylists() {
        // Placeholder until backend is ready
        playlists = ["Favorites", "Action", "RPG"];
    }

    function saveWizardSelections() {
        const selections = {
            category: wizardCategory,
            platform: wizardPlatform,
            emulator: wizardEmulator,
            fileAction: wizardFileAction,
            mediaOptions: $state.snapshot(wizardMediaOptions)
        };
        localStorage.setItem("wizardSelections", JSON.stringify(selections));
    }

    function loadWizardSelections() {
        const saved = localStorage.getItem("wizardSelections");
        if (saved) {
            try {
                const s = JSON.parse(saved);
                wizardCategory = s.category || "Consoles";
                wizardPlatform = s.platform || "";
                wizardEmulator = s.emulator || null;
                wizardFileAction = s.fileAction || "link";
                if (s.mediaOptions) {
                    Object.assign(wizardMediaOptions, s.mediaOptions);
                }
            } catch (e) { console.error("Failed to load wizard selections", e); }
        }
    }

    let wizardImportResults = $state([]);
    let wizardProgress = $state("");
    let wizardDetail = $state("");
    let installingStatus = $state("");

    let githubBuildStatus = $state({ status: "", conclusion: "", version: "" });

    const STANDARD_CATEGORIES = ["Consoles", "Handhelds", "Arcade", "Computers"];

    const STANDARD_PLATFORMS = [
        "Sony Playstation", "Sony Playstation 2", "Sony Playstation 3", "Sony Playstation Portable",
        "Nintendo Entertainment System", "Super Nintendo Entertainment System", "Nintendo 64", 
        "Nintendo GameCube", "Nintendo Wii", "Nintendo Wii U", "Nintendo Switch",
        "Nintendo Game Boy", "Nintendo Game Boy Color", "Nintendo Game Boy Advance", "Nintendo DS", "Nintendo 3DS",
        "Sega Genesis", "Sega Saturn", "Sega Dreamcast", "Sega Master System", "Sega Game Gear",
        "Arcade", "MAME", "SNK Neo Geo AES", "Atari 2600", "Atari 5200", "Atari 7800", "PC", "Windows"
    ];

    const CURRENT_VERSION = "v0.1.173";

    let suppressSave = false;
    async function saveLastState() {
        if (suppressSave) return;
        try {
            const platId = selectedPlatform?.id || null;
            const gameId = selectedGames[selectedGames.length - 1] || null;
            addLog(`[STATE] Saving last state: platform=${platId}, game=${gameId}`);
            config.last_platform_id = platId;
            config.last_game_id = gameId;
            await invoke("save_config", { config });
        } catch (e) {
            console.error("Failed to save state:", e);
        }
    }

    const INSTANCE_ID = Math.random().toString(36).substring(7);

    async function pushLogToServer(message: string, level = "INFO") {
        try {
            fetch("http://192.168.1.51:3002/log", {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    level,
                    source: `JS-FRONTEND-${INSTANCE_ID}`,
                    message
                })
            }).catch(() => {});
        } catch (e) {}
    }

    function addLog(message: string) {
        const timestamp = new Date().toLocaleTimeString();
        logs = [{ time: timestamp, message }, ...logs].slice(0, 100);
        console.log(`[JS LOG] ${message}`);
        if (config.data_root) {
            invoke("log_to_nas", { message, nas_path: config.data_root });
        }
        pushLogToServer(message);
    }

    async function loadConfig() {
        try {
            config = await invoke("get_config");
            if (!config.data_root) {
                setupWizardOpen = true;
            } else {
                addLog("Master data root active: " + config.data_root);
            }
        } catch (e) {
            addLog("Failed to load config: " + e);
        }
    }

    async function setMasterFolder() {
        const selected = await open({ directory: true, multiple: false });
        if (selected) {
            try {
                await invoke("set_data_root", { path: selected });
                addLog("Master folder set. Relaunching to apply...");
                await relaunch();
            } catch (e) {
                alert("Failed to set master folder: " + e);
            }
        }
    }

    async function loadPlatforms() {
        loading = true;
        try {
            addLog("Invoking load_library...");
            await invoke("load_library");
            addLog("Invoking get_platforms...");
            const raw = await invoke("get_platforms");
            addLog(`UI received ${raw.length} platforms. Updating state...`);
            // Explicitly set state to trigger reactivity
            platforms = [...raw];
            
            // Auto-select first platform if none selected
            if (platforms.length > 0 && !selectedPlatform) {
                const first = platforms[0];
                addLog(`Auto-selecting first platform: ${first.name}`);
                selectPlatform(first);
            }
        } catch (e) {
            addLog("Failed to load platforms: " + e);
        } finally {
            loading = false;
        }
    }

    async function loadEmulators() {
        try {
            emulators = await invoke("get_emulators");
        } catch (e) {
            addLog("Failed to load emulators: " + e);
        }
    }

    async function runAutoEmulatorSetup() {
        if (!config.data_root) return;
        emulatorSetupOpen = true;
        installingStatus = "Preparing folders...";
        try {
            await invoke("setup_emulator_environment", { masterPath: config.data_root });
            
            const toInstall = [
                { id: "retroarch", name: "RetroArch" },
                { id: "pcsx2", name: "PCSX2" },
                { id: "rpcs3", name: "RPCS3" },
                { id: "xemu", name: "xemu (Xbox)" }
            ];

            for (const emu of toInstall) {
                installingStatus = `Installing ${emu.name}...`;
                addLog(`Auto-installing ${emu.name}...`);
                await invoke("install_emulator", { emuId: emu.id, masterPath: config.data_root });
                addLog(`${emu.name} installed successfully.`);
            }
            
            installingStatus = "All emulators installed!";
            await loadEmulators();
        } catch (e) {
            addLog("Auto-setup failed: " + e);
            installingStatus = "Error: " + e;
        }
    }

    async function selectPlatform(platform) {
        selectedPlatform = platform;
        currentView = "library";
        loading = true;
        saveLastState();
        try {
            const rawGames = await invoke("get_games_for_platform", { platformId: platform.id });
            // Explicitly set state to trigger reactivity
            games = [...rawGames];
            addLog(`Loaded ${games.length} games for ${platform.name}.`);
            for (const game of games) {
                loadThumbnail(game);
            }
            loadPlatformEmulators(platform.id);
        } catch (e) {
            addLog("Failed to load games: " + e);
        } finally {
            loading = false;
        }
    }

    async function loadPlatformEmulators(platformId) {
        try {
            platformEmulators = await invoke("get_platform_emulators", { platformId });
        } catch (e) {
            console.error(e);
        }
    }

    async function loadThumbnail(game) {
        if (thumbnails[game.id]) return;
        const platform = platforms.find(p => p.id === game.platform_id);
        const mediaRoot = platform?.media_root || config.global_media_root || config.data_root || "";
        if (!mediaRoot) return;

        const types = IMAGE_GROUPS[activeImageGroup] || ["Box - 3D", "Box - Front"];
        const extensions = ["png", "jpg", "jpeg"];
        const cacheDir = `${mediaRoot}/Cache/Thumbnails`;
        const sTitle = sanitizeFilename(game.title);

        for (const type of types) {
            for (const ext of extensions) {
                try {
                    const sourcePath = `${mediaRoot}/Images/${platform.name}/${type}/${sTitle}-01.${ext}`;
                    const cachePath = await invoke("generate_thumbnail", {
                        sourcePath,
                        gameId: game.id,
                        cacheDir,
                        width: 300,
                        height: 400
                    });
                    thumbnails[game.id] = `game-media://localhost${cachePath}`;
                    return;
                } catch (e) {
                    continue;
                }
            }
        }
    }

    async function checkForUpdates() {
        if (isUpdating || isChecking) return;
        isChecking = true;
        updateStatus = "Checking...";
        const checkStartTime = Date.now();
        
        // Refresh build status too
        await checkGithubBuildStatus();
        
        try {
            addLog("Update engine: Checking for updates...");
            if (config.data_root) {
                invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: null });
            }
            
            // MANUAL UPDATE CHECK
            const update = await invoke("manual_check_update", { currentVersion: CURRENT_VERSION });
            
            lastChecked = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
            
            if (update) {
                addLog(`Update engine: Found v${update.version}!`);
                updateError = ""; 
                isUpdating = true;
                updateStatus = `Downloading v${update.version}...`;
                
                try {
                    addLog(`Downloading update from: ${update.url}`);
                    await invoke("manual_install_update", { url: update.url });
                    updateStatus = "Installing...";
                } catch (err) {
                    const errMsg = `Install failed: ${err}`;
                    addLog(errMsg);
                    updateError = errMsg;
                    if (config.data_root) invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: errMsg });
                    isUpdating = false; isChecking = false;
                    updateStatus = "";
                }
            } else {
                addLog("Update engine: Already at latest version.");
                const elapsed = Date.now() - checkStartTime;
                if (elapsed < 1000) await new Promise(r => setTimeout(r, 1000 - elapsed));
                updateStatus = "";
            }
        } catch (e) {
            addLog(`Update engine error: ${e}`);
            lastChecked = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }) + " (Failed)";
            updateStatus = "";
            if (!e?.toString().includes("404")) {
                const errMsg = `Check failed: ${e}`;
                updateError = errMsg;
                if (config.data_root) invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: errMsg });
            }
        } finally {
            isChecking = false;
        }
    }

    async function handleFileDrop(paths) {
        addLog(`[WIZARD] handleFileDrop called with ${paths.length} items: ${paths.join(', ')}`);
        wizardFiles = paths;
        importWizardOpen = true;
        wizardStep = 1;
        await loadEmulators();
    }

    async function startIndexing() {
        globalProgress = { active: true, title: "Indexing Metadata", detail: "Reading Metadata.xml...", percent: 0 };
        addLog("Starting background metadata indexing (this may take a few minutes)...");
        try {
            const unlisten = await listen('index-progress', (event) => {
                const p = event.payload;
                globalProgress.detail = `Processed ${p.count} games...`;
                globalProgress.percent = p.percent;
            });

            const result = await invoke("index_metadata");
            addLog(result);
            unlisten();
        } catch (e) {
            addLog("Indexing failed: " + e);
        } finally {
            globalProgress.active = false;
        }
    }

        async function updateSingleGameMetadata(gameId, runIndexer = true) {
            addLog(`Triggering update for game: ${gameId}`);
            const gameIndex = games.findIndex(g => g.id === gameId);
            if (gameIndex === -1) {
                addLog(`Error: Could not find game ${gameId} in local state.`);
                return;
            }
            
            const game = games[gameIndex];
    
            if (runIndexer) {
                addLog("Refreshing local metadata index first...");
                await invoke("index_metadata");
            }
    
            globalProgress = { active: true, title: "Updating Metadata", detail: `Scraping ${game.title}...`, percent: 0 };
            try {
                addLog(`Starting metadata scrape for ${game.title}...`);
                const platform = platforms.find(p => p.id === game.platform_id);
                const scrapedData = await invoke("scrape_game_art", { platform: platform?.name || "", title: game.title });
                
                // Save Metadata to DB
                await invoke("update_game_metadata", { gameId, data: scrapedData });
                
                // DOWNLOAD ART IMMEDIATELY (Detect extension)
                const masterRoot = config.data_root || config.global_media_root;
                if (masterRoot && scrapedData.art.box_3d_url) {
                    const url = scrapedData.art.box_3d_url;
                    const ext = url.split('.').pop().split('?')[0] || 'png';
                    const sTitle = sanitizeFilename(game.title);
                    const dest = `${masterRoot}/Images/${platform.name}/Box - 3D/${sTitle}-01.${ext}`;
                    globalProgress.detail = `Downloading 3D Box art...`;
                    await invoke("download_art", { url, destinationPath: dest });
                }
    
                // Small delay for NAS sync
                await new Promise(r => setTimeout(r, 1000));
    
                // RE-LOAD GAME DATA FROM DB TO ENSURE REACTIVITY
                const updatedGames = await invoke("get_games_for_platform", { platformId: game.platform_id });
                if (updatedGames) {
                    games = updatedGames;
                    // Force derived values to re-calculate by resetting selectedGames array
                    const lastSelected = [...selectedGames];
                    selectedGames = [];
                    setTimeout(() => { selectedGames = lastSelected; }, 10);
                    
                    thumbnails[gameId] = null; // Clear thumb to force reload
                    const newGameData = games.find(g => g.id === gameId);
                    if (newGameData) loadThumbnail(newGameData); 
                }
    
                addLog(`Metadata and Art updated successfully for ${game.title}`);
            } catch (e) {
                addLog(`Failed to update metadata for ${game.title}: ${e}`);
                console.error(e);
            } finally {
                globalProgress.active = false;
            }
        }
    

    async function runMultiUpdate() {
        addLog(`Triggering Batch Update for ${selectedGames.length} games.`);
        loading = true;
        multiUpdateStep = 2; // Show progress
        wizardProgress = "Starting batch update...";
        
        const gamesToUpdate = games.filter(g => selectedGames.includes(g.id));
        addLog(`Batch Update: Running for ${gamesToUpdate.length} games.`);

        try {
            for (let i = 0; i < gamesToUpdate.length; i++) {
                const game = gamesToUpdate[i];
                const platform = platforms.find(p => p.id === game.platform_id);
                wizardProgress = `Updating Game ${i + 1}/${gamesToUpdate.length}`;
                wizardDetail = `Scraping: ${game.title}`;
                addLog(`Batch: Processing ${game.title}...`);

                try {
                    const scrapedData = await invoke("scrape_game_art", { 
                        platform: platform?.name || "", 
                        title: game.title 
                    });
                    const scraped = scrapedData.art;

                    // Save Metadata to DB
                    await invoke("update_game_metadata", { gameId: game.id, data: scrapedData });
                    
                    const masterRoot = config.data_root || config.global_media_root;
                    if (!masterRoot) throw new Error("No data root or global media root configured.");

                    // Map of art types to download
                    const mediaMap = [
                        { opt: 'box_3d', url: scraped.box_3d_url, folder: 'Images', sub: 'Box - 3D', ext: 'png' },
                        { opt: 'box_front', url: scraped.box_front_url, folder: 'Images', sub: 'Box - Front', ext: 'png' },
                        { opt: 'box_back', url: scraped.box_back_url, folder: 'Images', sub: 'Box - Back', ext: 'png' },
                        { opt: 'clear_logo', url: scraped.clear_logo_url, folder: 'Images', sub: 'Clear Logo', ext: 'png' },
                        { opt: 'fanart_background', url: scraped.fanart_background_url, folder: 'Images', sub: 'Fanart - Background', ext: 'png' },
                    ];

                    for (const m of mediaMap) {
                        if (m.url) {
                            const sTitle = sanitizeFilename(game.title);
                            const dest = `${masterRoot}/${m.folder}/${platform?.name}/${m.sub}/${sTitle}-01.${m.ext}`;
                            
                            // Skip if exists AND replace is false
                            const skip = !multiUpdateReplace && await invoke("file_exists", { path: dest });
                            
                            if (!skip) {
                                wizardDetail = `Downloading ${m.sub} for ${game.title}...`;
                                await invoke("download_art", { url: m.url, destinationPath: dest });
                            }
                        }
                    }
                    
                    // Update local state so it refreshes immediately
                    thumbnails[game.id] = null;
                    loadThumbnail(game);
                } catch (err) {
                    addLog(`Batch update error for ${game.title}: ${err}`);
                }
            }
            
            addLog("Batch update complete!");
            multiUpdateStep = 3; // Finished
        } catch (e) {
            addLog("Batch update FAILED: " + e);
            alert("Batch update failed: " + e);
            multiUpdateWizardOpen = false;
        } finally {
            loading = false;
            wizardProgress = "";
            wizardDetail = "";
        }
    }

    async function runWizardImport() {
        wizardProgress = "Initializing...";
        wizardDetail = "Preparing directories...";
        saveWizardSelections();
        
        addLog(`[WIZARD] Starting Import for platform: ${wizardPlatform} (${wizardFiles.length} folders/files)`);
        try {
            // 1. Scaffold directories on NAS
            if (config.data_root) {
                addLog(`[WIZARD] Scaffolding NAS directories for ${wizardPlatform}...`);
                wizardDetail = "Scaffolding NAS directories...";
                await invoke("scaffold_platform_directories", { 
                    masterPath: config.data_root, 
                    platformId: wizardPlatform,
                    category: wizardCategory
                });
            }

            // 2. Link Emulator if selected
            if (wizardEmulator) {
                addLog(`[WIZARD] Linking emulator ${wizardEmulator} to ${wizardPlatform}...`);
                wizardDetail = "Linking emulator...";
                await invoke("link_platform_emulator", {
                    platformId: wizardPlatform,
                    emulatorId: wizardEmulator,
                    isDefault: true
                });
            }

            // 3. Perform Batch Import with File Action
            const results = [];
            for (let i = 0; i < wizardFiles.length; i++) {
                const path = wizardFiles[i];
                addLog(`[WIZARD] Processing item ${i+1}/${wizardFiles.length}: ${path}`);
                wizardProgress = `Processing Folder ${i + 1}/${wizardFiles.length}`;
                wizardDetail = `Analyzing: ${path.split(/[\\\\/]/).pop()}`;
                
                addLog(`[WIZARD] Invoking batch_import for path: ${path}`);
                const res = await invoke("batch_import", {
                    folderPath: path,
                    platformId: wizardPlatform,
                    category: wizardCategory,
                    fileAction: wizardFileAction,
                    mediaRoot: null 
                });
                addLog(`[WIZARD] batch_import found ${res.length} games.`);
                
                // 4. Detailed Media Scraping & Downloading
                for (let j = 0; j < res.length; j++) {
                    const title = res[j];
                    addLog(`[WIZARD] Scraping and Downloading for: ${title}`);
                    wizardProgress = `Processing Folder ${i + 1}/${wizardFiles.length}`;
                    wizardDetail = `Scraping: ${title} (${j+1}/${res.length})`;
                    
                    try {
                        addLog(`[WIZARD] Invoking scrape_game_art for ${title}...`);
                        const scrapedData = await invoke("scrape_game_art", { platform: wizardPlatform, title });
                        const scraped = scrapedData.art;
                        const masterRoot = config.data_root || config.global_media_root;

                        // Save Metadata to DB
                        const gameId = `${wizardPlatform}-${title}`.replace(/ /g, "-").toLowerCase();
                        addLog(`[WIZARD] Saving metadata for ${gameId}...`);
                        await invoke("update_game_metadata", { gameId, data: scrapedData });
                        
                        // Map Wizard Options to Scraped URLs and NAS Paths
                        // ... mediaMap definition ...
                        const mediaMap = [
                            { opt: 'box_3d', url: scraped.box_3d_url, folder: 'Images', sub: 'Box - 3D', ext: 'png' },
                            { opt: 'box_front', url: scraped.box_front_url, folder: 'Images', sub: 'Box - Front', ext: 'png' },
                            { opt: 'box_back', url: scraped.box_back_url, folder: 'Images', sub: 'Box - Back', ext: 'png' },
                            { opt: 'box_full', url: scraped.box_full_url, folder: 'Images', sub: 'Box - Full', ext: 'png' },
                            { opt: 'box_front_reconstructed', url: scraped.box_front_reconstructed_url, folder: 'Images', sub: 'Box - Front Reconstructed', ext: 'png' },
                            { opt: 'box_back_reconstructed', url: scraped.box_back_reconstructed_url, folder: 'Images', sub: 'Box - Back Reconstructed', ext: 'png' },
                            { opt: 'flyer_front', url: scraped.flyer_front_url, folder: 'Images', sub: 'Advertisement Flyer - Front', ext: 'png' },
                            { opt: 'flyer_back', url: scraped.flyer_back_url, folder: 'Images', sub: 'Advertisement Flyer - Back', ext: 'png' },
                            { opt: 'banner', url: scraped.banner_url, folder: 'Images', sub: 'Banner', ext: 'png' },
                            { opt: 'clear_logo', url: scraped.clear_logo_url, folder: 'Images', sub: 'Clear Logo', ext: 'png' },
                            { opt: 'fanart_background', url: scraped.fanart_background_url, folder: 'Images', sub: 'Fanart - Background', ext: 'png' },
                            { opt: 'disc', url: scraped.disc_url, folder: 'Images', sub: 'Disc', ext: 'png' },
                            { opt: 'cart_3d', url: scraped.cart_3d_url, folder: 'Images', sub: 'Cart - 3D', ext: 'png' },
                            { opt: 'cart_front', url: scraped.cart_front_url, folder: 'Images', sub: 'Cart - Front', ext: 'png' },
                            { opt: 'cart_back', url: scraped.cart_back_url, folder: 'Images', sub: 'Cart - Back', ext: 'png' },
                            { opt: 'arcade_cabinet', url: scraped.arcade_cabinet_url, folder: 'Images', sub: 'Arcade - Cabinet', ext: 'png' },
                            { opt: 'arcade_marquee', url: scraped.arcade_marquee_url, folder: 'Images', sub: 'Arcade - Marquee', ext: 'png' },
                            { opt: 'arcade_board', url: scraped.arcade_board_url, folder: 'Images', sub: 'Arcade - Circuit Board', ext: 'png' },
                            { opt: 'arcade_control_panel', url: scraped.arcade_control_panel_url, folder: 'Images', sub: 'Arcade - Control Panel', ext: 'png' },
                            { opt: 'arcade_controls_info', url: scraped.arcade_controls_info_url, folder: 'Images', sub: 'Arcade - Controls Info', ext: 'png' },
                            { opt: 'screenshot_gameplay', url: scraped.screenshot_gameplay_url, folder: 'Images', sub: 'Screenshot - Gameplay', ext: 'png' },
                            { opt: 'screenshot_title', url: scraped.screenshot_title_url, folder: 'Images', sub: 'Screenshot - Game Title', ext: 'png' },
                            { opt: 'screenshot_select', url: scraped.screenshot_select_url, folder: 'Images', sub: 'Screenshot - Game Select', ext: 'png' },
                            { opt: 'screenshot_gameover', url: scraped.screenshot_gameover_url, folder: 'Images', sub: 'Screenshot - Game Over', ext: 'png' },
                            { opt: 'screenshot_scores', url: scraped.screenshot_scores_url, folder: 'Images', sub: 'Screenshot - High Scores', ext: 'png' },
                            { opt: 'bigbox_video', url: scraped.bigbox_video_url, folder: 'Videos', sub: 'Video - Big Box Cinematic', ext: 'mp4' },
                            { opt: 'gameplay_video', url: scraped.gameplay_video_url, folder: 'Videos', sub: 'Video - Gameplay', ext: 'mp4' }
                        ];

                        for (const m of mediaMap) {
                            if (wizardMediaOptions[m.opt] && m.url) {
                                addLog(`[WIZARD] Triggering download for ${m.sub}: ${m.url}`);
                                wizardDetail = `Downloading ${m.sub} for ${title}...`;
                                const sTitle = sanitizeFilename(title);
                                const dest = `${masterRoot}/${m.folder}/${wizardPlatform}/${m.sub}/${sTitle}-01.${m.ext}`;
                                await invoke("download_art", { url: m.url, destinationPath: dest });
                            }
                        }
                    } catch (scrapeErr) {
                        addLog(`Download failed for ${title}: ${scrapeErr}`);
                    }
                }
                results.push(...res);
            }
            addLog(`[WIZARD] Import workflow finished successfully for ${results.length} games.`);
            wizardImportResults = results;
            wizardStep = 5; // Success!
            addLog(`[WIZARD] Triggering UI reload...`);
            await loadPlatforms();
        } catch (e) {
            addLog("[WIZARD] CRITICAL ERROR: " + e);
            alert("Import failed: " + e);
        } finally {
            loading = false;
            wizardProgress = "";
            wizardDetail = "";
        }
    }

    async function playGame(gameId) {
        try {
            addLog(`Attempting to launch game ${gameId}...`);
            await invoke("launch_game", { gameId });
        } catch (e) {
            addLog("Launch failed: " + e);
            alert("Launch failed: " + e);
        }
    }

    async function linkEmulator(emuId) {
        if (!selectedPlatform) return;
        try {
            await invoke("link_platform_emulator", {
                platformId: selectedPlatform.id,
                emulatorId: emuId,
                isDefault: true
            });
            addLog(`Linked ${emuId} to ${selectedPlatform.name}`);
            loadPlatformEmulators(selectedPlatform.id);
        } catch (e) {
            addLog("Failed to link emulator: " + e);
        }
    }

    async function deletePlatform(platformId) {
        if (!confirm("Are you sure you want to delete this platform and all its games?")) return;
        try {
            await invoke("delete_platform", { platformId });
            addLog(`Deleted platform: ${platformId}`);
            selectedPlatform = null;
            platformMenuOpen = false;
            await loadPlatforms();
        } catch (e) {
            addLog("Failed to delete platform: " + e);
        }
    }

    async function resetUpdateState() {
        isUpdating = false; isChecking = false;
        updateStatus = "";
        lastChecked = "Resetting...";
        addLog("Manual Update Reset triggered.");
        checkForUpdates();
    }

    async function checkGithubBuildStatus() {
        try {
            githubBuildStatus = await invoke("get_build_status");
        } catch (e) {
            console.error("Failed to check build status:", e);
        }
    }

    function closeAllMenus() {
        menuOpen = false;
        toolsMenuOpen = false;
        downloadMenuOpen = false;
        viewMenuOpen = false;
        imageGroupMenuOpen = false;
        platformMenuOpen = false;
        closeContextMenu();
    }

    function handleClickOutside(e) {
        const isMenuClick = e.target.closest('.menu-dropdown') || e.target.closest('.hamburger');
        const isContextClick = e.target.closest('.context-menu');
        const isPlatformMenuClick = e.target.closest('.platform-menu-wrap');

        if (!isMenuClick) {
            menuOpen = false;
            toolsMenuOpen = false;
            downloadMenuOpen = false;
            viewMenuOpen = false;
            imageGroupMenuOpen = false;
        }
        if (!isContextClick) {
            contextMenu.open = false;
        }
        if (!isPlatformMenuClick) {
            platformMenuOpen = false;
        }
    }

    onMount(() => {
        addLog("App mounting...");
        window.addEventListener('click', handleClickOutside);
        
        // GLOBAL ERROR HANDLER
        window.onerror = (msg, url, line, col, error) => {
            pushLogToServer(`JS CRASH: ${msg} at ${line}:${col}`, "ERROR");
            addLog(`JS Error: ${msg}`);
            return false;
        };

        loadWizardSelections();
        
        // Load active image group
        const savedGroup = localStorage.getItem("activeImageGroup");
        if (savedGroup && savedGroup in IMAGE_GROUPS) {
            activeImageGroup = savedGroup;
        }
        
        checkGithubBuildStatus();
        const buildStatusInterval = setInterval(checkGithubBuildStatus, 60000);

        // Periodic check for updates every 30 seconds
        const updateInterval = setInterval(checkForUpdates, 30000);
        
        // Initial check after 5 seconds
        setTimeout(checkForUpdates, 5000);

        // Async background tasks
        (async () => {
            try {
                addLog("Starting Drag & Drop listener...");
                const unlisten = await getCurrentWindow().onDragDropEvent((event) => {
                    const type = event.type || (event.payload && event.payload.type);
                    const paths = event.paths || (event.payload && event.payload.paths);

                    if (type === 'drop' || type === 'dragDrop') {
                        const pathCount = paths ? paths.length : 0;
                        addLog(`[DRAG-DEBUG] Final Drop detected! Type: ${type}, Items: ${pathCount}`);
                        if (paths) {
                            paths.forEach((p, i) => addLog(`[DRAG-DEBUG] Drop Path ${i+1}: ${p}`));
                            addLog(`[DRAG-DEBUG] Calling handleFileDrop with ${paths.length} paths.`);
                            handleFileDrop(paths);
                        } else {
                            addLog(`[DRAG-DEBUG] Drop event fired but no paths found in payload.`);
                        }
                        isDragging = false;
                    } else if (type === 'enter' || type === 'dragEnter') {
                        addLog(`[DRAG-DEBUG] Item entered window area. Type: ${type}`);
                        isDragging = true;
                    } else if (type === 'over' || type === 'dragOver') {
                    } else if (type === 'leave' || type === 'dragLeave') {
                        addLog(`[DRAG-DEBUG] Item left window area. Type: ${type}`);
                        isDragging = false;
                    } else {
                        addLog(`[DRAG-DEBUG] Other Drag event: ${type}`);
                        isDragging = false;
                    }
                });
                
                // Set up library-ready listener BEFORE loadConfig to guarantee we
                // never miss the event. The backend emits this after DB init + library
                // load completes, which can take 5+ seconds. Calling load_library before
                // the SqlitePool is managed causes a CRITICAL PANIC in the backend.
                let libraryReadyHandled = false;
                async function handleLibraryReady() {
                    if (libraryReadyHandled) return;
                    libraryReadyHandled = true;
                    addLog("Library-ready: fetching platforms from RAM...");
                    try {
                        // get_platforms reads from the Library mutex (managed at app start),
                        // safe to call without worrying about SqlitePool initialization state.
                        const raw = await invoke("get_platforms");
                        addLog(`UI received ${raw.length} platforms. Updating state...`);
                        platforms = [...raw];
                        if (platforms.length > 0 && !selectedPlatform) {
                            addLog(`Auto-selecting first platform: ${platforms[0].name}`);
                            selectPlatform(platforms[0]);
                        }
                        // RESTORE STATE
                        if (config.last_platform_id) {
                            addLog(`[STATE] Attempting to restore platform: ${config.last_platform_id}`);
                            const lastPlat = platforms.find(p => p.id === config.last_platform_id);
                            if (lastPlat) {
                                suppressSave = true;
                                await selectPlatform(lastPlat);
                                if (config.last_game_id) {
                                    addLog(`[STATE] Attempting to restore game: ${config.last_game_id}`);
                                    setTimeout(() => {
                                        selectedGames = [config.last_game_id];
                                        suppressSave = false;
                                    }, 100);
                                } else {
                                    suppressSave = false;
                                }
                            } else {
                                addLog(`[STATE] Could not find platform ${config.last_platform_id} in loaded platforms.`);
                            }
                        }
                        if (config.data_root) {
                            await invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: null });
                        }
                    } catch (e) {
                        addLog(`Library-ready handler error: ${e}`);
                    }
                }

                const unlistenLibraryReady = await listen("library-ready", async () => {
                    await handleLibraryReady();
                    unlistenLibraryReady();
                });

                await loadConfig();
                if (!config.data_root) {
                    addLog("Warning: No data_root found during onMount.");
                } else {
                    addLog("Config loaded. Waiting for library-ready signal...");
                    // Fallback: if library-ready fired before our listener was registered
                    // (very fast DB init), poll after 10s and call get_platforms directly.
                    setTimeout(async () => {
                        if (!libraryReadyHandled) {
                            addLog("Fallback: library-ready not received, polling get_platforms...");
                            await handleLibraryReady();
                        }
                    }, 10000);
                }
            } catch (err) {
                addLog(`Startup background error: ${err}`);
            }
        })();

        return () => {
            window.removeEventListener('click', handleClickOutside);
            clearInterval(updateInterval);
            clearInterval(buildStatusInterval);
        };
    });
</script>

<div class="app">
    {#if isDragging}
        <div class="drop-overlay">
            <div class="drop-card">
                <div class="icon">📥</div>
                <h2>Drop ROMs to Import</h2>
                <p>TurboLaunch will automatically setup art and emulators.</p>
            </div>
        </div>
    {/if}

    <aside class="sidebar">
        <div class="header">
            <button class="hamburger" onclick={() => menuOpen = !menuOpen} aria-label="Menu">
                <span class="bar"></span>
                <span class="bar"></span>
                <span class="bar"></span>
            </button>
            <div class="title-wrap">
                <h2>TurboLaunch</h2>
                <span class="version-tag">{CURRENT_VERSION}</span>
            </div>
        </div>

        {#if menuOpen}
            <div class="menu-dropdown">
                <button onclick={() => { currentView = 'emulators'; menuOpen = false; loadEmulators(); }}>Emulator Settings</button>
                
                <div class="menu-item-container">
                    <button class="menu-sub-trigger" onclick={() => toolsMenuOpen = !toolsMenuOpen}>
                        Tools <span class="arrow">{toolsMenuOpen ? '◀' : '▶'}</span>
                    </button>
                    {#if toolsMenuOpen}
                        <div class="menu-submenu">
                            <div class="menu-item-container">
                                <button class="menu-sub-trigger" onclick={() => downloadMenuOpen = !downloadMenuOpen}>
                                    Download <span class="arrow">{downloadMenuOpen ? '◀' : '▶'}</span>
                                </button>
                                {#if downloadMenuOpen}
                                    <div class="menu-submenu side">
                                        <button onclick={() => { 
                                            if (selectedGames.length > 0) {
                                                multiUpdateWizardOpen = true; 
                                                multiUpdateStep = 1;
                                                menuOpen = false; 
                                            } else {
                                                alert("Please select at least one game first.");
                                            }
                                        }}>
                                            Update Metadata and Media for Selected Games
                                        </button>
                                        <button onclick={() => { startIndexing(); menuOpen = false; }}>
                                            Index Metadata Database
                                        </button>
                                    </div>
                                {/if}
                            </div>
                            <button onclick={() => { currentView = 'tools'; menuOpen = false; }}>Paths & Environment</button>
                        </div>
                    {/if}
                </div>

                <div class="menu-item-container">
                    <button class="menu-sub-trigger" onclick={() => viewMenuOpen = !viewMenuOpen}>
                        View <span class="arrow">{viewMenuOpen ? '◀' : '▶'}</span>
                    </button>
                    {#if viewMenuOpen}
                        <div class="menu-submenu">
                            <div class="menu-item-container">
                                <button class="menu-sub-trigger" onclick={() => imageGroupMenuOpen = !imageGroupMenuOpen}>
                                    Image Group <span class="arrow">{imageGroupMenuOpen ? '◀' : '▶'}</span>
                                </button>
                                {#if imageGroupMenuOpen}
                                    <div class="menu-submenu side">
                                        {#each Object.keys(IMAGE_GROUPS) as group}
                                            <button onclick={() => { setImageGroup(group); menuOpen = false; }}>
                                                {activeImageGroup === group ? '✓ ' : ''}{group}
                                            </button>
                                        {/each}
                                    </div>
                                {/if}
                            </div>
                        </div>
                    {/if}
                </div>

                <button onclick={() => { currentView = 'debug'; menuOpen = false; }}>Debug Logs</button>
                <button onclick={() => { currentView = 'library'; menuOpen = false; }}>Back to Library</button>
            </div>
        {/if}

        <nav class="platform-list">
            <h3>Platforms</h3>
            {#each [...new Set(platforms.map(p => p.category))] as category}
                <div class="category-group">
                    <h4>{category}</h4>
                    <ul>
                        {#each platforms.filter(p => p.category === category) as platform}
                            <li class="platform-item {selectedPlatform?.id === platform.id && currentView === 'library' ? 'active' : ''}">
                                <button class="platform-btn" onclick={() => { selectPlatform(platform); platformMenuOpen = false; }}>{platform.name}</button>
                                {#if selectedPlatform?.id === platform.id && currentView === 'library'}
                                    <div class="platform-menu-wrap">
                                        <button class="btn-dots" onclick={(e) => { e.stopPropagation(); platformMenuOpen = !platformMenuOpen }}>•••</button>
                                        {#if platformMenuOpen}
                                            <div class="platform-dropdown">
                                                <button class="btn-delete" onclick={() => deletePlatform(platform.id)}>Delete Platform</button>
                                            </div>
                                        {/if}
                                    </div>
                                {/if}
                            </li>
                        {/each}
                    </ul>
                </div>
            {/each}
        </nav>

        <div class="sidebar-footer">
            {#if updateError}
                <div class="update-error-msg" title={updateError}>
                    ⚠️ Update Failed
                    <button class="btn-tiny" onclick={resetUpdateState}>Retry</button>
                </div>
            {/if}
            <div class="update-status-minimal">
                <button class="mini-spinner-btn" onclick={checkForUpdates} 
                    title={githubBuildStatus.status === 'in_progress' || githubBuildStatus.status === 'queued' ? 'GitHub Build In Progress...' : 
                           githubBuildStatus.conclusion === 'failure' ? 'Last Build Failed' : 'Check for updates now'}>
                    <div class="mini-spinner" 
                        class:rotating={isUpdating || isChecking || githubBuildStatus.status === 'in_progress' || githubBuildStatus.status === 'queued'} 
                        class:building={githubBuildStatus.status === 'in_progress' || githubBuildStatus.status === 'queued'}
                        class:failed={githubBuildStatus.conclusion === 'failure'}>
                    </div>
                </button>
                <div class="update-info">
                    {#if updateStatus}
                        <span class="status-msg">{updateStatus}</span>
                    {:else}
                        <span class="check-time">Last check: {lastChecked || '...'}</span>
                    {/if}
                </div>
            </div>
        </div>
    </aside>

    <main class="content">
        {#if setupWizardOpen}
            <div class="wizard-overlay">
                <div class="wizard-card welcome-card">
                    <div class="icon">🚀</div>
                    <h1>Welcome to TurboLaunch</h1>
                    <p>Select a folder on your **NAS** to store your database, settings, and media. Everything will sync across all your machines.</p>
                    <button class="btn-primary btn-large" onclick={setMasterFolder}>Select Master NAS Folder</button>
                </div>
            </div>
        {/if}

        {#if emulatorSetupOpen}
            <div class="wizard-overlay">
                <div class="wizard-card">
                    <h2>Emulator Auto-Setup</h2>
                    <div class="spinner"></div>
                    <p>{installingStatus}</p>
                    {#if installingStatus === "All emulators installed!"}
                        <button class="btn-primary" onclick={() => emulatorSetupOpen = false}>Finish</button>
                    {/if}
                </div>
            </div>
        {/if}

        {#if updateStatus === "Restarting..."}
            <div class="update-overlay">
                <div class="update-card">
                    <div class="spinner"></div>
                    <h2>Refreshing...</h2>
                    <p>Installing update. Back in a second!</p>
                </div>
            </div>
        {/if}

        {#if importWizardOpen}
            <div class="wizard-overlay">
                <div class="wizard-card large-wizard">
                    <header class="wizard-header">
                        <h2>Import Games Wizard</h2>
                        <button class="btn-close" onclick={() => importWizardOpen = false}>&times;</button>
                    </header>
                    
                    <div class="wizard-progress-bar">
                        <div class="step {wizardStep >= 1 ? 'active' : ''}"><span>1</span><label>Platform</label></div>
                        <div class="step {wizardStep >= 2 ? 'active' : ''}"><span>2</span><label>Emulator</label></div>
                        <div class="step {wizardStep >= 3 ? 'active' : ''}"><span>3</span><label>Files</label></div>
                        <div class="step {wizardStep >= 4 ? 'active' : ''}"><span>4</span><label>Media</label></div>
                        <div class="step {wizardStep >= 5 ? 'active' : ''}"><span>5</span><label>Finish</label></div>
                    </div>

                    <div class="step-content">
                        {#if loading}
                            <div class="step-inner loading-step">
                                <div class="spinner-large"></div>
                                <h3>{wizardProgress}</h3>
                                <p>{wizardDetail}</p>
                            </div>
                        {:else if wizardStep === 1}
                            <div class="step-inner">
                                <h3>What platform are you importing games for?</h3>
                                <p>Select the category and platform that these games belong to.</p>
                                <div class="selection-row">
                                    <div class="selection-box half">
                                        <label>Category</label>
                                        <select bind:value={wizardCategory}>
                                            {#each STANDARD_CATEGORIES as c}
                                                <option value={c}>{c}</option>
                                            {/each}
                                        </select>
                                    </div>
                                    <div class="selection-box half">
                                        <label>Platform Name</label>
                                        <select bind:value={wizardPlatform}>
                                            <option value="">-- Select Platform --</option>
                                            {#each STANDARD_PLATFORMS as p}
                                                <option value={p}>{p}</option>
                                            {/each}
                                        </select>
                                    </div>
                                </div>
                                <div class="wizard-actions">
                                    <button class="btn-secondary" onclick={() => importWizardOpen = false}>Cancel</button>
                                    <button class="btn-primary" onclick={() => wizardStep = 2} disabled={!wizardPlatform}>Next &gt;</button>
                                </div>
                            </div>

                        {:else if wizardStep === 2}
                            <div class="step-inner">
                                <h3>Select an Emulator</h3>
                                <p>Which emulator should be used to launch these games?</p>
                                <div class="selection-box">
                                    <select bind:value={wizardEmulator} size="8">
                                        <option value={null}>-- None / Manual Setup --</option>
                                        {#each emulators as emu}
                                            <option value={emu.id}>{emu.name}</option>
                                        {/each}
                                    </select>
                                </div>
                                <div class="wizard-actions">
                                    <button class="btn-secondary" onclick={() => wizardStep = 1}>&lt; Back</button>
                                    <button class="btn-primary" onclick={() => wizardStep = 3}>Next &gt;</button>
                                </div>
                            </div>

                        {:else if wizardStep === 3}
                            <div class="step-inner">
                                <h3>File Management</h3>
                                <p>How should OpenBox handle your game files?</p>
                                <div class="radio-group">
                                    <label class="radio-card" class:selected={wizardFileAction === 'link'}>
                                        <input type="radio" bind:group={wizardFileAction} value="link" />
                                        <div class="radio-info">
                                            <strong>Use files in their current location</strong>
                                            <span>OpenBox will link to the files where they are now. Use this for NAS shares.</span>
                                        </div>
                                    </label>
                                    <label class="radio-card" class:selected={wizardFileAction === 'copy'}>
                                        <input type="radio" bind:group={wizardFileAction} value="copy" />
                                        <div class="radio-info">
                                            <strong>Copy files to my OpenBox games folder</strong>
                                            <span>Recommended. Creates a duplicate in {config.data_root}/Games.</span>
                                        </div>
                                    </label>
                                    <label class="radio-card" class:selected={wizardFileAction === 'move'}>
                                        <input type="radio" bind:group={wizardFileAction} value="move" />
                                        <div class="radio-info">
                                            <strong>Move files into my OpenBox games folder</strong>
                                            <span>Moves the files from their current location to your NAS.</span>
                                        </div>
                                    </label>
                                </div>
                                <div class="wizard-actions">
                                    <button class="btn-secondary" onclick={() => wizardStep = 2}>&lt; Back</button>
                                    <button class="btn-primary" onclick={() => wizardStep = 4}>Next &gt;</button>
                                </div>
                            </div>

                        {:else if wizardStep === 4}
                            <div class="step-inner">
                                <h3>Metadata & Media</h3>
                                <p>How would you like to download images for your games?</p>
                                <div class="media-grid-scroll">
                                    <div class="media-column">
                                        <h4>Marketing & Box Art</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.flyer_front} /> Advertisement Flyer - Front</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.flyer_back} /> Advertisement Flyer - Back</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.banner} /> Banner</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.clear_logo} /> Clear Logo</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.fanart_background} /> Fanart - Background</label>
                                        <div class="divider-small"></div>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_3d} /> Box - 3D</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_front} /> Box - Front</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_back} /> Box - Back</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_full} /> Box - Full</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_front_reconstructed} /> Box - Front Reconstructed</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_back_reconstructed} /> Box - Back Reconstructed</label>
                                    </div>
                                    <div class="media-column">
                                        <h4>Arcade & Hardware</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_cabinet} /> Arcade - Cabinet</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_board} /> Arcade - Circuit Board</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_control_panel} /> Arcade - Control Panel</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_controls_info} /> Arcade - Controls Info</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_marquee} /> Arcade - Marquee</label>
                                        <div class="divider-small"></div>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.disc} /> Disc</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.cart_3d} /> Cartridge - 3D</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.cart_front} /> Cartridge - Front</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.cart_back} /> Cartridge - Back</label>
                                    </div>
                                    <div class="media-column">
                                        <h4>Video & Screenshots</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.bigbox_video} /> Big Box Cinematic Video</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.gameplay_video} /> Gameplay Video</label>
                                        <div class="divider-small"></div>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.screenshot_gameplay} /> Screenshot - Gameplay</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.screenshot_title} /> Screenshot - Game Title</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.screenshot_select} /> Screenshot - Game Select</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.screenshot_gameover} /> Screenshot - Game Over</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.screenshot_scores} /> Screenshot - High Scores</label>
                                    </div>
                                </div>
                                <div class="wizard-actions">
                                    <button class="btn-secondary" onclick={() => wizardStep = 3}>&lt; Back</button>
                                    <button class="btn-primary" onclick={runWizardImport}>Import Now</button>
                                </div>
                            </div>

                        {:else if wizardStep === 5}
                            <div class="step-inner success-step">
                                <div class="icon-big">✅</div>
                                <h3>Import Complete!</h3>
                                <p>Successfully processed <strong>{wizardImportResults.length}</strong> games for <strong>{wizardPlatform}</strong>.</p>
                                <div class="import-summary">
                                    {#each wizardImportResults.slice(0, 5) as item}
                                        <div class="summary-item">imported: {item}</div>
                                    {/each}
                                    {#if wizardImportResults.length > 5}
                                        <div class="summary-more">...and {wizardImportResults.length - 5} more</div>
                                    {/if}
                                </div>
                                <div class="wizard-actions">
                                    <button class="btn-primary" onclick={() => importWizardOpen = false}>Finish</button>
                                </div>
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        {/if}

        {#if currentView === 'library'}
            {#if selectedPlatform}
                <div class="library-layout">
                    <div class="main-column">
                        <header class="view-header">
                            <h1>{selectedPlatform.name} <span class="count">({games.length})</span></h1>
                            {#if platformEmulators.length > 0}
                                <span class="emu-tag">Default: {platformEmulators[0].name}</span>
                            {/if}
                        </header>

                        <div class="scrollable-content" onclick={closeContextMenu}>
                            {#if favoriteGames.length > 0}
                                <h2 class="section-title">Favorites</h2>
                                <div class="game-grid">
                                    {#each favoriteGames as game}
                                        <div class="game-card {selectedGames.includes(game.id) ? 'selected' : ''}" 
                                             onclick={(e) => { e.stopPropagation(); toggleSelection(game.id, e); }}>
                                            <div class="thumbnail" ondblclick={() => playGame(game.id)} oncontextmenu={(e) => openContextMenu(e, game.id)}>
                                                {#if thumbnails[game.id]}
                                                    <img src={thumbnails[game.id]} alt={game.title} />
                                                {:else}
                                                    <div class="placeholder"><span>{game.title}</span></div>
                                                {/if}
                                                <div class="card-overlay">
                                                    <button class="btn-play-icon" onclick={() => playGame(game.id)}>▶</button>
                                                </div>
                                            </div>
                                            <div class="info">
                                                <h3>{game.title}</h3>
                                                {#if game.ra_game_id}
                                                    <span class="ra-tag" title="RetroAchievements Compatible">🏆</span>
                                                {/if}
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}

                            <h2 class="section-title">Games</h2>
                            <div class="game-grid">
                                {#each otherGames as game}
                                    <div class="game-card {selectedGames.includes(game.id) ? 'selected' : ''}" 
                                         onclick={(e) => { e.stopPropagation(); toggleSelection(game.id, e); }}>
                                        <div class="thumbnail" ondblclick={() => playGame(game.id)} oncontextmenu={(e) => openContextMenu(e, game.id)}>
                                            {#if thumbnails[game.id]}
                                                <img src={thumbnails[game.id]} alt={game.title} />
                                            {:else}
                                                <div class="placeholder"><span>{game.title}</span></div>
                                            {/if}
                                            <div class="card-overlay">
                                                <button class="btn-play-icon" onclick={() => playGame(game.id)}>▶</button>
                                            </div>
                                        </div>
                                        <div class="info">
                                            <h3>{game.title}</h3>
                                            {#if game.ra_game_id}
                                                <span class="ra-tag" title="RetroAchievements Compatible">🏆</span>
                                            {/if}
                                        </div>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    </div>
                </div>
            {:else}
                <div class="welcome-screen"><div class="icon">📦</div><h1>Drag & Drop ROMs Here</h1></div>
            {/if}
        {:else if currentView === 'emulators'}
            <div class="settings-view">
                <h1>Emulators</h1>
                <button class="btn-retroarch" onclick={runAutoEmulatorSetup}>🚀 Run Auto-Setup (RetroArch, PCSX2, RPCS3, xemu)</button>
                <div class="emulator-list">
                    <table>
                        <tbody>
                            {#each emulators as emu}
                                <tr>
                                    <td><strong>{emu.name}</strong></td>
                                    <td><button class="btn-small" onclick={() => linkEmulator(emu.id)}>Set Default for {selectedPlatform?.name || '...'}</button></td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            </div>
        {:else if currentView === 'tools'}
            <div class="settings-view">
                <h1>Tools & Paths</h1>
                <div class="setting-item">
                    <h3>Master NAS Folder</h3>
                    <input bind:value={config.data_root} readonly />
                    <button class="btn-primary" onclick={setMasterFolder}>Change</button>
                </div>

                <div class="setting-item">
                    <h3>Local Metadata Cache</h3>
                    <p style="font-size: 0.75rem; color: #888; margin-bottom: 10px;">
                        Refresh your local searchable database from <strong>NAS/Data/Metadata.xml</strong>.
                    </p>
                    <button class="btn-primary" onclick={startIndexing}>Index Metadata.xml</button>
                </div>

                <div class="setting-item">
                    <h3>RetroAchievements Integration</h3>
                    <div class="input-group">
                        <label>RA Username</label>
                        <input bind:value={config.ra_username} placeholder="Username" />
                    </div>
                    <div class="input-group">
                        <label>RA API Key</label>
                        <input bind:value={config.ra_api_key} type="password" placeholder="API Key" />
                    </div>
                    <button class="btn-primary" onclick={async () => {
                        try {
                            await invoke("save_config", { config });
                            addLog("Configuration saved successfully.");
                        } catch (e) { addLog("Save failed: " + e); }
                    }}>Save Settings</button>
                </div>
            </div>
        {:else if currentView === 'debug'}
            <div class="debug-view">
                <header class="debug-header">
                    <h1>Debug Logs</h1>
                    <button class="btn-small" onclick={resetUpdateState}>Reset Update Engine</button>
                </header>
                <div class="log-container">
                {#each logs as log}<div class="log-entry"><span class="log-time">[{log.time}]</span><span class="log-msg">{log.message}</span></div>{/each}
            </div></div>
        {/if}
    </main>

    {#if selectedGame && currentView === 'library'}
        <aside class="details-pane">
            <div class="details-header">
                {#if selectedGameMedia.video}
                    <video src={selectedGameMedia.video} class="background" autoplay loop muted playsinline></video>
                {:else if selectedGameMedia.screenshots.length > 0}
                    <img src={selectedGameMedia.screenshots[0]} class="background" alt="bg" />
                {/if}
                <div class="logo-overlay">
                    {#if selectedGameMedia.logo}
                        <img src={selectedGameMedia.logo} alt="logo" />
                    {:else}
                        <h1 class="fallback-logo">{selectedGame.title}</h1>
                    {/if}
                </div>
            </div>

            <div class="details-body">
                <div class="details-top-actions">
                    <div class="star-rating">
                        {#each [1, 2, 3, 4, 5] as star}
                            <button class="star { (selectedGame.star_rating || 0) >= star ? 'active' : '' }" 
                                    onclick={() => setRating(selectedGame.id, star)}>
                                ★
                            </button>
                        {/each}
                    </div>
                    <button class="btn-heart {selectedGame.favorite ? 'active' : ''}" 
                            onclick={() => toggleFavorite(selectedGame.id)}>
                        ❤
                    </button>
                </div>

                <h1 class="details-title">{selectedGame.title}</h1>

                <div class="metadata-list">
                    <div class="metadata-item">
                        <span class="label">Platform</span>
                        <span class="value">{selectedPlatform?.name || '...'}</span>
                    </div>
                    {#if selectedGame.release_date}
                        <div class="metadata-item">
                            <span class="label">Released</span>
                            <span class="value">{selectedGame.release_date.split('T')[0]}</span>
                        </div>
                    {/if}
                    {#if selectedGame.developer}
                        <div class="metadata-item">
                            <span class="label">Developer</span>
                            <span class="value">{selectedGame.developer}</span>
                        </div>
                    {/if}
                    {#if selectedGame.genre}
                        <div class="metadata-item">
                            <span class="label">Genre</span>
                            <span class="value">{selectedGame.genre}</span>
                        </div>
                    {/if}
                    {#if selectedGame.play_mode}
                        <div class="metadata-item">
                            <span class="label">Play Mode</span>
                            <span class="value">{selectedGame.play_mode}</span>
                        </div>
                    {/if}
                </div>

                {#if selectedGame.description}
                    <div class="details-description">
                        {selectedGame.description}
                    </div>
                {/if}

                <div class="media-gallery">
                    {#each selectedGameMedia.screenshots.slice(0, 4) as ss}
                        <div class="gallery-item">
                            <img src={ss} alt="screenshot" />
                        </div>
                    {/each}
                </div>
            </div>
        </aside>
    {/if}

    {#if globalProgress.active}
        <footer class="global-footer">
            <div class="progress-container">
                <div class="progress-bar" style="width: {globalProgress.percent}%"></div>
                <div class="progress-text">
                    <span class="title">{globalProgress.title}</span>
                    <span class="detail">{globalProgress.detail}</span>
                    <span class="percent">{globalProgress.percent}%</span>
                </div>
            </div>
        </footer>
    {/if}

    {#if contextMenu.open}
        <div class="context-menu" style="top: {contextMenu.y}px; left: {contextMenu.x}px;" onclick={(e) => e.stopPropagation()}>
            <button onclick={() => { playGame(contextMenu.gameId); closeContextMenu(); }}>
                Play... {games.find(g => g.id === contextMenu.gameId)?.ra_game_id ? '🏆' : ''}
            </button>
            <div class="menu-item-with-sub">
                <button>Play version <span class="arrow">▶</span></button>
                <div class="menu-sub-dropdown">
                    {#if contextMenuVersions.length > 0}
                        {#each contextMenuVersions as v}
                            <button onclick={() => { playGame(v.id); closeContextMenu(); }}>
                                {v.title} {v.ra_game_id ? '👑' : ''}
                            </button>
                        {/each}
                        <div class="menu-divider"></div>
                        <button onclick={async () => {
                            try {
                                addLog(`Checking RA compatibility for ${contextMenu.gameId}...`);
                                await invoke("check_ra_compatibility", { gameId: contextMenu.gameId });
                                // Refresh versions
                                const game = games.find(g => g.id === contextMenu.gameId);
                                if (game) contextMenuVersions = await invoke("get_game_versions", { title: game.title });
                            } catch (e) { addLog("RA Check failed: " + e); }
                        }}>Verify Achievement Compatibility</button>
                    {:else}
                        <button disabled>No other versions found</button>
                    {/if}
                </div>
            </div>
            <div class="menu-item-with-sub">
                <button>Launch with <span class="arrow">▶</span></button>
                <div class="menu-sub-dropdown">
                    {#each emulators as emu}
                        <button onclick={() => { /* TODO: Launch with specific emu */ }}>{emu.name}</button>
                    {/each}
                </div>
            </div>
            <button>Open RetroArch</button>
            <button>RetroArch Netplay</button>
            <div class="menu-divider"></div>
            <button onclick={() => { updateSingleGameMetadata(contextMenu.gameId); closeContextMenu(); }}>Update Metadata & Media...</button>
            <div class="menu-item-with-sub">
                <button>Add to Favorites <span class="arrow">▶</span></button>
                <div class="menu-sub-dropdown">
                    <button onclick={() => { showPlaylistSlideout = true; closeContextMenu(); }}>+ Add Playlist</button>
                    {#each playlists as pl}
                        <button>{pl}</button>
                    {/each}
                </div>
            </div>
            <button onclick={() => { invoke('reset_game_stats', { gameId: contextMenu.gameId }); closeContextMenu(); }}>Reset play count and time</button>
            <button onclick={closeContextMenu}>Reset last played</button>
            <button onclick={closeContextMenu}>Expand selected games</button>
            <button class="btn-menu-delete" onclick={() => { invoke('delete_game', { gameId: contextMenu.gameId }); closeContextMenu(); }}>Delete</button>
            <div class="menu-divider"></div>
            <div class="menu-item-with-sub">
                <button>Media <span class="arrow">▶</span></button>
                <div class="menu-sub-dropdown">
                    <button>View images</button>
                    <button>View 3D box model</button>
                    <button>Play music</button>
                    <button>Flip box</button>
                    <button>Save image as...</button>
                    <button>Refresh selected images</button>
                </div>
            </div>
            <div class="menu-item-with-sub">
                <button>File Management <span class="arrow">▶</span></button>
                <div class="menu-sub-dropdown">
                    <button onclick={() => {
                        const game = games.find(g => g.id === contextMenu.gameId);
                        if (game) invoke('open_folder', { path: game.file_path });
                        closeContextMenu();
                    }}>Open game folder</button>
                    <button onclick={closeContextMenu}>Open images folder</button>
                </div>
            </div>
            <button onclick={closeContextMenu}>Select ROM in archive</button>
            <button onclick={async () => {
                try {
                    const path = await invoke('generate_m3u', { gameId: contextMenu.gameId });
                    addLog(`Successfully generated M3U: ${path}`);
                } catch (e) {
                    addLog(`M3U generation failed: ${e}`);
                }
                closeContextMenu();
            }}>Batch cache games (Generate M3U)</button>
        </div>
    {/if}

    {#if multiUpdateWizardOpen}
        <div class="wizard-overlay">
            <div class="wizard-card wide">
                <div class="wizard-header">
                    <h2>Update Metadata & Media</h2>
                    <button class="btn-close" onclick={() => multiUpdateWizardOpen = false}>&times;</button>
                </div>

                <div class="step-content">
                    {#if multiUpdateStep === 1}
                        <div class="step-inner">
                            <p>You have selected <strong>{selectedGames.length}</strong> games to update.</p>
                            <p>How would you like to handle existing metadata and media?</p>
                            
                            <div class="selection-list">
                                <label class="selection-item {multiUpdateReplace === false ? 'active' : ''}">
                                    <input type="radio" name="replace" value={false} bind:group={multiUpdateReplace} />
                                    <div class="selection-text">
                                        <span class="title">Yes, do not replace any existing fields or media (recommended)</span>
                                        <span class="desc">Only missing information and images will be downloaded.</span>
                                    </div>
                                </label>

                                <label class="selection-item {multiUpdateReplace === true ? 'active' : ''}">
                                    <input type="radio" name="replace" value={true} bind:group={multiUpdateReplace} />
                                    <div class="selection-text">
                                        <span class="title">Download and replace all existing metadata and media</span>
                                        <span class="desc">Everything will be re-scraped and overwritten with fresh data.</span>
                                    </div>
                                </label>
                            </div>

                            <div class="wizard-actions">
                                <button class="btn-secondary" onclick={() => multiUpdateWizardOpen = false}>Cancel</button>
                                <button class="btn-primary" onclick={runMultiUpdate}>Update Games Now</button>
                            </div>
                        </div>

                    {:else if multiUpdateStep === 2}
                        <div class="step-inner loading-step">
                            <div class="spinner-large"></div>
                            <h3>{wizardProgress}</h3>
                            <p>{wizardDetail}</p>
                        </div>

                    {:else if multiUpdateStep === 3}
                        <div class="step-inner success-step">
                            <div class="success-icon">✅</div>
                            <h3>Batch Update Complete!</h3>
                            <p>Successfully processed {selectedGames.length} games.</p>
                            <div class="wizard-actions">
                                <button class="btn-primary" onclick={() => multiUpdateWizardOpen = false}>Finish</button>
                            </div>
                        </div>
                    {/if}
                </div>
            </div>
        </div>
    {/if}
</div>

<style>
    :global(body) { margin: 0; padding: 0; background: #121212; color: #e0e0e0; font-family: 'Segoe UI', system-ui, sans-serif; }
    
    .context-menu { position: fixed; background: #222; border: 1px solid #333; border-radius: 8px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); z-index: 3000; min-width: 220px; padding: 5px 0; animation: menuFade 0.1s ease-out; }
    .context-menu button { width: 100%; padding: 8px 15px; background: none; border: none; color: #ddd; text-align: left; cursor: pointer; font-size: 0.85rem; display: flex; justify-content: space-between; align-items: center; }
    .context-menu button:hover { background: #3b82f6; color: white; }
    .context-menu .arrow { font-size: 0.7rem; opacity: 0.5; }
    .btn-menu-delete:hover { background: #ef4444 !important; }
    .menu-divider { height: 1px; background: #333; margin: 5px 0; }
    
    .menu-item-with-sub { position: relative; }
    .menu-sub-dropdown { position: absolute; left: 100%; top: -5px; background: #222; border: 1px solid #333; border-radius: 8px; box-shadow: 0 10px 30px rgba(0,0,0,0.5); min-width: 200px; padding: 5px 0; display: none; }
    .menu-item-with-sub:hover > .menu-sub-dropdown { display: block; }
    .menu-sub-dropdown.side { left: 100%; top: 0; }

    @keyframes menuFade { from { opacity: 0; transform: scale(0.95); } to { opacity: 1; transform: scale(1); } }

    .app { display: flex; height: 100vh; overflow: hidden; position: relative; }
    .sidebar { width: 280px; background: #181818; padding: 20px; border-right: 1px solid #282828; display: flex; flex-direction: column; gap: 20px; position: relative; z-index: 50; }
    .sidebar .header { display: flex; align-items: center; gap: 15px; }
    .hamburger { background: none; border: none; cursor: pointer; display: flex; flex-direction: column; gap: 4px; }
    .hamburger .bar { display: block; width: 20px; height: 2px; background: #fff; border-radius: 2px; }
    .title-wrap { display: flex; flex-direction: column; }
    .version-tag { font-size: 0.6rem; color: #555; font-weight: bold; margin-top: -2px; }
    .sidebar h2 { margin: 0; font-size: 1.1rem; font-weight: 600; color: #fff; }
    .menu-dropdown { position: absolute; top: 60px; left: 20px; width: 240px; background: #282828; border: 1px solid #383838; border-radius: 8px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); z-index: 100; }
    .menu-dropdown button { width: 100%; padding: 12px 15px; background: none; border: none; color: #ddd; text-align: left; cursor: pointer; font-size: 0.9rem; }
    .menu-dropdown button:hover { background: #383838; color: #fff; }
    .sidebar-footer { margin-top: auto; padding-top: 10px; border-top: 1px solid #222; }
    .update-status-minimal { display: flex; align-items: center; gap: 10px; color: #555; font-size: 0.65rem; }
    
    .update-error-msg {
        background: rgba(239, 68, 68, 0.1);
        color: #ef4444;
        font-size: 0.65rem;
        padding: 8px;
        border-radius: 4px;
        margin-bottom: 8px;
        display: flex;
        justify-content: space-between;
        align-items: center;
        border: 1px solid rgba(239, 68, 68, 0.2);
    }

    .btn-tiny {
        background: #ef4444;
        color: white;
        border: none;
        padding: 2px 6px;
        border-radius: 3px;
        cursor: pointer;
        font-size: 0.6rem;
    }

    .drop-overlay {
        position: absolute; top: 0; left: 0; right: 0; bottom: 0;
        background: rgba(59, 130, 246, 0.2);
        backdrop-filter: blur(10px);
        z-index: 5000;
        display: flex; align-items: center; justify-content: center;
        border: 4px dashed #3b82f6;
        margin: 10px; border-radius: 20px;
        pointer-events: none;
    }
    .drop-card {
        background: #1e1e1e; padding: 40px; border-radius: 20px;
        text-align: center; border: 1px solid #333;
        box-shadow: 0 20px 50px rgba(0,0,0,0.5);
    }
    .drop-card .icon { font-size: 4rem; margin-bottom: 20px; }

    .mini-spinner-btn { background: none; border: none; padding: 0; margin: 0; cursor: pointer; display: flex; align-items: center; justify-content: center; }
    .mini-spinner-btn:hover .mini-spinner { border-left-color: #fff; }
    .mini-spinner { width: 10px; height: 10px; border: 2px solid rgba(255, 255, 255, 0.05); border-left-color: #333; border-radius: 50%; }
    .mini-spinner.rotating { border-left-color: #3b82f6; animation: spin 1s linear infinite; }
    .mini-spinner.building { border-left-color: #f59e0b; }
    .mini-spinner.failed { border-left-color: #ef4444; }
    .update-info { display: flex; flex-direction: column; }
    .status-msg { color: #3b82f6; font-weight: bold; }
    @keyframes spin { to { transform: rotate(360deg); } }
    .platform-list { flex: 1; overflow-y: auto; }
    .platform-list h3 { font-size: 0.75rem; text-transform: uppercase; color: #555; margin-bottom: 10px; }
    .category-group { margin-bottom: 20px; }
    .category-group h4 { font-size: 0.7rem; text-transform: uppercase; color: #444; margin: 0 0 8px 10px; border-left: 2px solid #333; padding-left: 10px; }
    .sidebar ul { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 4px; }
    .sidebar li button { width: 100%; padding: 8px 12px; background: none; border: none; color: #aaa; text-align: left; cursor: pointer; border-radius: 4px; font-size: 0.85rem; }
    .sidebar li.active button { background: #3b82f6; color: white; }
    .content { flex: 1; display: flex; flex-direction: column; background: #121212; position: relative; }
    .game-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 20px; }
    .game-card { background: #181818; border-radius: 8px; overflow: hidden; border: 2px solid transparent; text-align: left; padding: 0; cursor: pointer; color: inherit; transition: border-color 0.2s; }
    .game-card.selected { border-color: #3b82f6; box-shadow: 0 0 15px rgba(59, 130, 246, 0.4); }
    .thumbnail { aspect-ratio: 3/4; background: #222; display: flex; align-items: center; justify-content: center; text-align: center; position: relative; }
    .thumbnail img { width: 100%; height: 100%; object-fit: cover; }
    .card-overlay { position: absolute; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; opacity: 0; transition: opacity 0.2s; }
    .game-card:hover .card-overlay { opacity: 1; }
    .btn-play-icon { width: 50px; height: 50px; border-radius: 50%; background: #3b82f6; color: white; border: none; font-size: 1.5rem; cursor: pointer; display: flex; align-items: center; justify-content: center; padding-left: 5px; }
    .info { padding: 12px; position: relative; }
    .info h3 { margin: 0; font-size: 0.85rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding-right: 20px; }
    .ra-tag { position: absolute; top: 12px; right: 10px; font-size: 0.8rem; cursor: help; }
    .update-overlay, .wizard-overlay { position: fixed; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.9); display: flex; align-items: center; justify-content: center; z-index: 2000; backdrop-filter: blur(5px); }
    .update-card, .wizard-card { background: #1e1e1e; padding: 40px; border-radius: 16px; border: 1px solid #333; box-shadow: 0 20px 60px rgba(0,0,0,0.8); text-align: center; max-width: 500px; }
    .spinner { width: 60px; height: 60px; border: 4px solid rgba(59, 130, 246, 0.1); border-left-color: #3b82f6; border-radius: 50%; margin: 0 auto; animation: spin 1s linear infinite; }
    .btn-primary { background: #3b82f6; color: white; border: none; padding: 10px 20px; border-radius: 6px; cursor: pointer; font-weight: 600; }
    .btn-large { padding: 15px 30px; font-size: 1.1rem; }
    .btn-retroarch { background: #df4a1f; color: white; border: none; padding: 12px; border-radius: 6px; width: 100%; cursor: pointer; font-weight: 600; margin: 10px 0; }
    .or-divider { margin: 10px 0; color: #444; font-size: 0.8rem; font-weight: bold; }
    .steps { display: flex; gap: 10px; margin-bottom: 20px; }
    .step { flex: 1; height: 4px; background: #333; border-radius: 2px; }
    .step.active { background: #3b82f6; }
    .log-container { background: #000; padding: 20px; border-radius: 8px; height: 500px; overflow-y: auto; font-family: monospace; font-size: 0.85rem; border: 1px solid #222; }
    .log-entry { margin-bottom: 5px; border-bottom: 1px solid #111; }
    .log-msg { color: #0f0; }
    .welcome-screen { display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; opacity: 0.5; }
    .emu-tag { background: #333; color: #aaa; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; margin-top: 5px; display: inline-block; }
    .art-options { display: flex; flex-direction: column; gap: 15px; margin: 20px 0; text-align: left; }
    table { width: 100%; border-collapse: collapse; margin-top: 20px; }
    td { padding: 12px; border-bottom: 1px solid #282828; }

    /* Large Wizard Overhaul */
    .large-wizard { width: 800px; max-width: 90vw; max-height: 85vh; display: flex; flex-direction: column; }
    .wizard-progress-bar { display: flex; justify-content: space-between; padding: 20px 40px; background: #181818; border-bottom: 1px solid #333; }
    .wizard-progress-bar .step { flex: 1; text-align: center; position: relative; color: #555; }
    .wizard-progress-bar .step span { width: 30px; height: 30px; border-radius: 50%; background: #333; display: flex; align-items: center; justify-content: center; margin: 0 auto 8px; font-weight: bold; border: 2px solid transparent; z-index: 2; position: relative; }
    .wizard-progress-bar .step label { font-size: 0.7rem; text-transform: uppercase; font-weight: 600; }
    .wizard-progress-bar .step.active { color: #3b82f6; }
    .wizard-progress-bar .step.active span { background: #3b82f6; color: white; border-color: #1e1e1e; }
    .wizard-progress-bar .step::after { content: ''; position: absolute; top: 15px; left: 50%; width: 100%; height: 2px; background: #333; z-index: 1; }
    .wizard-progress-bar .step:last-child::after { display: none; }
    
    .step-content { flex: 1; overflow-y: auto; padding: 30px 40px; text-align: left; }
    .step-inner h3 { margin-bottom: 10px; color: #fff; font-size: 1.4rem; }
    .step-inner p { color: #aaa; margin-bottom: 25px; }
    
    .selection-row { display: flex; gap: 20px; margin-bottom: 20px; }
    .selection-box.half { flex: 1; }
    .selection-box label { display: block; font-size: 0.7rem; text-transform: uppercase; color: #555; margin-bottom: 8px; font-weight: bold; }
    .selection-box select { width: 100%; background: #121212; border: 1px solid #333; color: white; border-radius: 8px; padding: 10px; outline: none; }
    .selection-box option { padding: 10px; border-bottom: 1px solid #181818; }
    .selection-box option:hover { background: #3b82f6; }

    .radio-group { display: flex; flex-direction: column; gap: 15px; }
    .radio-card { display: flex; align-items: center; gap: 20px; background: #181818; border: 1px solid #333; padding: 20px; border-radius: 12px; cursor: pointer; transition: all 0.2s; }
    .radio-card:hover { border-color: #444; background: #1c1c1c; }
    .radio-card.selected { border-color: #3b82f6; background: rgba(59, 130, 246, 0.1); }
    .radio-card strong { display: block; font-size: 1rem; color: #fff; }
    .radio-card span { font-size: 0.85rem; color: #888; }
    .radio-card input { transform: scale(1.5); }

    .media-grid-scroll { display: grid; grid-template-columns: repeat(3, 1fr); gap: 20px; background: #121212; padding: 20px; border-radius: 12px; border: 1px solid #333; }
    .media-column h4 { font-size: 0.8rem; color: #3b82f6; text-transform: uppercase; margin-bottom: 15px; border-bottom: 1px solid #333; padding-bottom: 5px; }
    .media-column .checkbox { display: flex; align-items: center; gap: 10px; font-size: 0.85rem; margin-bottom: 10px; cursor: pointer; color: #ddd; }
    .divider-small { height: 1px; background: #222; margin: 15px 0; }
    .metadata-toggle { margin-bottom: 30px; background: #181818; padding: 20px; border-radius: 12px; border: 1px solid #333; }
    .checkbox-large { display: flex; align-items: center; gap: 15px; font-weight: bold; color: #fff; cursor: pointer; }
    .checkbox-large input { width: 20px; height: 20px; }

    .wizard-actions { margin-top: 40px; display: flex; justify-content: flex-end; gap: 15px; border-top: 1px solid #333; padding-top: 25px; }
    
    .loading-step { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 40px 0; text-align: center; }
    .spinner-large { width: 50px; height: 50px; border: 4px solid rgba(59, 130, 246, 0.1); border-top: 4px solid #3b82f6; border-radius: 50%; animation: spin 1s linear infinite; margin-bottom: 20px; }
    @keyframes spin { 0% { transform: rotate(0deg); } 100% { transform: rotate(360deg); } }
    .btn-secondary { background: #333; color: #eee; border: none; padding: 10px 25px; border-radius: 6px; cursor: pointer; font-weight: 600; }
    .btn-secondary:hover { background: #444; }
    .success-step { text-align: center; }
    .icon-big { font-size: 5rem; margin-bottom: 20px; }
    .import-summary { background: #000; padding: 20px; border-radius: 8px; font-family: monospace; font-size: 0.8rem; text-align: left; max-height: 200px; overflow-y: auto; color: #0f0; border: 1px solid #222; }
    .summary-item { margin-bottom: 4px; }
    .summary-more { color: #888; margin-top: 10px; font-style: italic; }
    .platform-item { position: relative; display: flex; align-items: center; }
    .platform-btn { flex: 1; text-align: left; padding: 8px 12px; background: none; border: none; color: #aaa; cursor: pointer; border-radius: 4px; font-size: 0.85rem; }
    .platform-item.active .platform-btn { background: #3b82f6; color: white; }
    .platform-menu-wrap { position: absolute; right: 5px; display: flex; align-items: center; }
    .btn-dots { background: none; border: none; color: rgba(255,255,255,0.5); cursor: pointer; padding: 5px; font-size: 0.8rem; }
    .btn-dots:hover { color: #fff; }
    .platform-dropdown { position: absolute; top: 100%; right: 0; background: #282828; border: 1px solid #383838; border-radius: 4px; box-shadow: 0 5px 15px rgba(0,0,0,0.5); z-index: 1000; min-width: 150px; }
    .menu-item-container { position: relative; }
    .menu-sub-trigger { display: flex; justify-content: space-between; align-items: center; }
    .menu-submenu { position: absolute; left: 100%; top: -1px; width: 220px; background: #282828; border: 1px solid #383838; border-left: 2px solid #3b82f6; border-radius: 0 8px 8px 8px; box-shadow: 10px 10px 25px rgba(0,0,0,0.5); z-index: 110; display: flex; flex-direction: column; }
    .menu-submenu.side { left: 100%; top: 0; }
    .selection-list { display: flex; flex-direction: column; gap: 15px; margin: 30px 0; text-align: left; }
    .selection-item { display: flex; gap: 15px; padding: 20px; background: #252525; border: 1px solid #333; border-radius: 12px; cursor: pointer; transition: all 0.2s; }
    .selection-item:hover { background: #2a2a2a; border-color: #444; }
    .selection-item.active { background: #1e293b; border-color: #3b82f6; }
    .selection-item input { width: 20px; height: 20px; margin-top: 2px; }
    .selection-text .title { display: block; font-weight: bold; color: #fff; font-size: 1rem; margin-bottom: 4px; }
    .selection-text .desc { display: block; font-size: 0.85rem; color: #aaa; }
    .wizard-card.wide { max-width: 700px; width: 90%; }
    .wizard-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 25px; border-bottom: 1px solid #333; padding-bottom: 15px; }
    .wizard-header h2 { margin: 0; font-size: 1.4rem; color: #fff; }
    .btn-close { background: none; border: none; color: #666; font-size: 1.8rem; cursor: pointer; line-height: 1; padding: 0; transition: color 0.2s; }
    .btn-close:hover { color: #fff; }
    .success-icon { font-size: 4rem; margin-bottom: 20px; }
    .btn-delete { width: 100%; padding: 10px; background: none; border: none; color: #ff4444; text-align: left; cursor: pointer; font-size: 0.8rem; }
    .btn-delete:hover { background: #383838; }

    .library-layout { display: flex; flex: 1; min-height: 0; overflow: hidden; }
    .main-column { flex: 1; display: flex; flex-direction: column; min-height: 0; }
    .scrollable-content { flex: 1; overflow-y: auto; padding: 20px; }
    .section-title { font-size: 1.2rem; font-weight: 700; color: #3b82f6; margin: 30px 0 15px 0; border-bottom: 1px solid #222; padding-bottom: 10px; }
    .section-title:first-child { margin-top: 0; }

    /* Narrow Sidebar Details Pane - Refined */
    .details-pane { width: 240px; flex-shrink: 0; background: #0f0f0f; border-left: 1px solid #222; overflow-y: auto; display: flex; flex-direction: column; height: 100vh; position: relative; z-index: 40; }
    .details-header { position: relative; width: 100%; aspect-ratio: 16/9; background: #000; overflow: hidden; border-bottom: 1px solid #222; flex-shrink: 0; }
    .details-header .background { width: 100%; height: 100%; object-fit: cover; opacity: 0.25; filter: blur(4px); }
    .details-header video.background { opacity: 0.6; filter: none; }
    .details-header .logo-overlay { position: absolute; top: 0; left: 0; width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; padding: 12px; }
    .details-header .logo-overlay img { max-width: 100%; max-height: 100%; object-fit: contain; filter: drop-shadow(0 4px 8px rgba(0,0,0,0.9)); }
    .fallback-logo { font-size: 0.9rem; font-weight: 900; text-align: center; color: #fff; text-transform: uppercase; letter-spacing: 0.5px; opacity: 0.8; }

    .details-body { padding: 12px; }
    .details-top-actions { display: flex; justify-content: space-between; align-items: center; margin-bottom: 12px; }
    .btn-heart { background: none; border: none; font-size: 1rem; cursor: pointer; color: #222; transition: all 0.2s; padding: 0; }
    .btn-heart:hover { transform: scale(1.2); color: #444; }
    .btn-heart.active { color: #ef4444; }
    .star-rating { display: flex; gap: 1px; }
    .star { background: none; border: none; padding: 0; font-size: 0.8rem; color: #1a1a1a; cursor: pointer; transition: color 0.2s; }
    .star.active { color: #facc15; }

    .details-title { font-size: 0.95rem; font-weight: 800; margin: 0 0 10px 0; color: #fff; line-height: 1.2; border-bottom: 1px solid #222; padding-bottom: 6px; }
    .metadata-list { display: flex; flex-direction: column; gap: 4px; margin-bottom: 12px; }
    .metadata-item { display: flex; font-size: 0.7rem; line-height: 1.2; }
    .metadata-item .label { width: 75px; color: #444; font-weight: 700; text-transform: uppercase; font-size: 0.55rem; letter-spacing: 0.5px; }
    .metadata-item .value { flex: 1; color: #999; }

    .details-description { font-size: 0.7rem; color: #777; line-height: 1.4; border-top: 1px solid #222; padding-top: 10px; max-height: 200px; overflow-y: auto; scrollbar-width: none; }
    .details-description::-webkit-scrollbar { display: none; }
    .media-gallery { display: grid; grid-template-columns: repeat(2, 1fr); gap: 4px; margin-top: 12px; }
    .gallery-item { aspect-ratio: 16/9; background: #000; border-radius: 3px; overflow: hidden; border: 1px solid #1a1a1a; }
    .gallery-item img { width: 100%; height: 100%; object-fit: cover; cursor: pointer; transition: opacity 0.2s; }
    .gallery-item img:hover { opacity: 0.6; }

    /* Global Progress Bar (Bottom) */
    .global-footer { position: fixed; bottom: 0; left: 0; right: 0; height: 36px; background: #181818; border-top: 1px solid #282828; z-index: 4000; display: flex; align-items: center; padding: 0 15px; }
    .progress-container { flex: 1; height: 20px; background: #0a0a0a; border-radius: 10px; position: relative; overflow: hidden; border: 1px solid #333; }
    .progress-bar { position: absolute; top: 0; left: 0; height: 100%; background: linear-gradient(90deg, #3b82f6, #60a5fa); transition: width 0.3s ease; }
    .progress-text { position: absolute; top: 0; left: 0; width: 100%; height: 100%; display: flex; align-items: center; padding: 0 12px; font-size: 0.65rem; font-weight: 700; gap: 12px; z-index: 2; }
    .progress-text .title { color: #fff; text-transform: uppercase; }
    .progress-text .detail { color: #888; flex: 1; font-weight: 400; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    .progress-text .percent { color: #fff; }

    .settings-view { padding: 40px; max-width: 800px; }
    .setting-item { margin-bottom: 40px; background: #181818; padding: 25px; border-radius: 12px; border: 1px solid #282828; }
    .setting-item h3 { margin: 0 0 15px 0; font-size: 1.1rem; color: #3b82f6; }
    .input-group { margin-bottom: 15px; }
    .input-group label { display: block; font-size: 0.7rem; text-transform: uppercase; color: #555; margin-bottom: 5px; font-weight: bold; }
    .input-group input { width: 100%; background: #0a0a0a; border: 1px solid #333; color: #fff; padding: 10px; border-radius: 6px; outline: none; }
    .input-group input:focus { border-color: #3b82f6; }
    .setting-item input[readonly] { width: 100%; background: #0a0a0a; border: 1px solid #222; color: #666; padding: 10px; border-radius: 6px; margin-bottom: 10px; }
</style>
