<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { open } from "@tauri-apps/plugin-dialog";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { check } from '@tauri-apps/plugin-updater';
    import { relaunch } from '@tauri-apps/plugin-process';

    let platforms = $state([]);
    let selectedPlatform = $state(null);
    let games = $state([]);
    let loading = $state(false);
    const defaultNasPath = "/home/ubuntubox/freenas/Emulation/Aaron Program Files (x86)/LaunchBox";
    let launchboxRoot = $state(defaultNasPath); 
    let currentView = $state("library"); 
    let menuOpen = $state(false);
    let updateStatus = $state("");

    let emulators = $state([]);
    let newEmulator = $state({ id: "", name: "", executable_path: "", default_cmdline: "" });
    let thumbnails = $state({}); 

    // Wizard State
    let wizardOpen = $state(false);
    let wizardStep = $state(1); // 1: Platform, 2: Media Source, 3: Scraper, 4: Import
    let wizardFiles = $state([]);
    let wizardPlatform = $state(null);
    let wizardMediaMode = $state("standalone"); // "standalone" or "launchbox"
    let wizardCustomMediaRoot = $state("");
    let wizardScrape3D = $state(true);
    let wizardImportResults = $state([]);

    async function loadPlatforms() {
        loading = true;
        try {
            await invoke("load_library");
            platforms = await invoke("get_platforms");
        } catch (e) {
            console.error("Failed to load platforms", e);
        } finally {
            loading = false;
        }
    }

    async function selectPlatform(platform) {
        selectedPlatform = platform;
        currentView = "library";
        loading = true;
        try {
            games = await invoke("get_games_for_platform", { platformId: platform.id });
            for (const game of games) {
                loadThumbnail(game);
            }
        } catch (e) {
            console.error("Failed to load games", e);
        } finally {
            loading = false;
        }
    }

    async function loadThumbnail(game) {
        if (thumbnails[game.id]) return;
        
        // Find platform media root
        const platform = platforms.find(p => p.id === game.platform_id);
        const mediaRoot = platform?.media_root || launchboxRoot;

        const types = ["Box - 3D", "Box - Front"];
        const extensions = ["png", "jpg", "jpeg"];
        const cacheDir = `${mediaRoot}/Cache/Thumbnails`;

        for (const type of types) {
            for (const ext of extensions) {
                try {
                    const sourcePath = `${mediaRoot}/Images/${game.platform_id}/${type}/${game.title}-01.${ext}`;
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

    async function autoDetect() {
        loading = true;
        try {
            const detectedPath = await invoke("detect_launchbox");
            if (detectedPath) {
                launchboxRoot = detectedPath;
                await loadPlatforms();
            }
        } catch (e) {
            console.error("Auto-detection failed", e);
        } finally {
            loading = false;
        }
    }

    async function checkForUpdates() {
        try {
            const update = await check();
            if (update) {
                updateStatus = `Update v${update.version} available! Downloading...`;
                await update.downloadAndInstall();
                updateStatus = "Update installed. Relaunching...";
                await relaunch();
            }
        } catch (e) {
            console.error("Update check failed", e);
        }
    }

    async function handleFileDrop(paths) {
        wizardFiles = paths;
        wizardOpen = true;
        wizardStep = 1;
        if (selectedPlatform) wizardPlatform = selectedPlatform.id;
    }

    async function pickMediaRoot() {
        const selected = await open({ directory: true, multiple: false });
        if (selected) wizardCustomMediaRoot = selected;
    }

    async function runWizardImport() {
        wizardStep = 4;
        loading = true;
        try {
            const results = [];
            for (const path of wizardFiles) {
                const res = await invoke("batch_import", {
                    folderPath: path,
                    platformId: wizardPlatform,
                    mediaRoot: wizardMediaMode === "launchbox" ? wizardCustomMediaRoot : null
                });
                results.push(...res);
            }
            
            // TODO: If wizardCustomMediaRoot is set, update platform's media_root in DB
            
            wizardImportResults = results;
            await loadPlatforms();
        } catch (e) {
            console.error("Wizard import failed", e);
        } finally {
            loading = false;
        }
    }

    async function scrapeGame(game) {
        try {
            const scraped = await invoke("scrape_game_art", {
                platform: selectedPlatform.name,
                title: game.title
            });
            if (scraped.box_3d_url) {
                const mediaRoot = selectedPlatform?.media_root || launchboxRoot;
                const dest = `${mediaRoot}/Images/${game.platform_id}/Box - 3D/${game.title}-01.png`;
                await invoke("download_art", { url: scraped.box_3d_url, destinationPath: dest });
                delete thumbnails[game.id];
                loadThumbnail(game);
            }
        } catch (e) {
            console.error("Scrape failed", e);
        }
    }

    onMount(async () => {
        autoDetect();
        checkForUpdates();
        
        const unlisten = await getCurrentWindow().onFileDropEvent((event) => {
            if (event.payload.type === 'drop') {
                handleFileDrop(event.payload.paths);
            }
        });

        return () => unlisten();
    });
</script>

<div class="app">
    <aside class="sidebar">
        <div class="header">
            <button class="hamburger" onclick={() => menuOpen = !menuOpen} aria-label="Menu">
                <span class="bar"></span>
                <span class="bar"></span>
                <span class="bar"></span>
            </button>
            <h2>TurboLaunch</h2>
        </div>

        {#if menuOpen}
            <div class="menu-dropdown">
                <button onclick={() => { currentView = 'emulators'; menuOpen = false; }}>Emulator Settings</button>
                <button onclick={() => { wizardOpen = true; wizardStep = 1; menuOpen = false; }}>Import Wizard</button>
                <button onclick={() => { currentView = 'tools'; menuOpen = false; }}>Tools</button>
                <button onclick={() => { currentView = 'library'; menuOpen = false; }}>Back to Library</button>
            </div>
        {/if}

        <div class="actions">
            <button onclick={autoDetect} disabled={loading} class="btn-primary">
                {loading ? 'Detecting...' : 'Auto-Detect'}
            </button>
        </div>

        <div class="current-path" title={launchboxRoot}>
            Path: {launchboxRoot.split('/').pop()}
        </div>

        {#if updateStatus}
            <div class="update-banner">
                {updateStatus}
            </div>
        {/if}

        <nav class="platform-list">
            <h3>Platforms</h3>
            <ul>
                {#each platforms as platform}
                    <li class={selectedPlatform?.id === platform.id && currentView === 'library' ? 'active' : ''}>
                        <button onclick={() => selectPlatform(platform)}>{platform.name}</button>
                    </li>
                {/each}
            </ul>
        </nav>
    </aside>

    <main class="content">
        {#if wizardOpen}
            <div class="wizard-overlay">
                <div class="wizard-card">
                    <header>
                        <h2>Import Wizard</h2>
                        <button class="btn-close" onclick={() => wizardOpen = false} aria-label="Close">&times;</button>
                    </header>

                    <div class="steps">
                        <div class="step {wizardStep >= 1 ? 'active' : ''}">1. Platform</div>
                        <div class="step {wizardStep >= 2 ? 'active' : ''}">2. Media</div>
                        <div class="step {wizardStep >= 3 ? 'active' : ''}">3. Scraper</div>
                        <div class="step {wizardStep >= 4 ? 'active' : ''}">4. Finish</div>
                    </div>

                    <div class="step-content">
                        {#if wizardStep === 1}
                            <p>Select the platform for <strong>{wizardFiles.length}</strong> items:</p>
                            <select bind:value={wizardPlatform}>
                                <option value={null}>Select a platform...</option>
                                {#each platforms as p}
                                    <option value={p.id}>{p.name}</option>
                                {/each}
                            </select>
                            <div class="wizard-actions">
                                <button onclick={() => wizardStep = 2} disabled={!wizardPlatform}>Next &rarr;</button>
                            </div>
                        {:else if wizardStep === 2}
                            <p>Where is the media for this platform located?</p>
                            <div class="radio-group">
                                <label>
                                    <input type="radio" value="standalone" bind:group={wizardMediaMode} />
                                    Stand-alone (TurboLaunch managed)
                                </label>
                                <label>
                                    <input type="radio" value="launchbox" bind:group={wizardMediaMode} />
                                    Existing LaunchBox Installation
                                </label>
                            </div>

                            {#if wizardMediaMode === "launchbox"}
                                <div class="path-picker">
                                    <input bind:value={wizardCustomMediaRoot} placeholder="LaunchBox Root Folder" readonly />
                                    <button onclick={pickMediaRoot}>Locate</button>
                                </div>
                            {/if}

                            <div class="wizard-actions">
                                <button class="btn-back" onclick={() => wizardStep = 1}>&larr; Back</button>
                                <button onclick={() => wizardStep = 3}>Next &rarr;</button>
                            </div>
                        {:else if wizardStep === 3}
                            <p>Scraping options:</p>
                            <label class="checkbox">
                                <input type="checkbox" bind:checked={wizardScrape3D} />
                                Download 3D Boxes automatically
                            </label>
                            <div class="wizard-actions">
                                <button class="btn-back" onclick={() => wizardStep = 2}>&larr; Back</button>
                                <button class="btn-primary" onclick={runWizardImport}>Import Now</button>
                            </div>
                        {:else if wizardStep === 4}
                            <div class="results">
                                {#if loading}
                                    <div class="loader">Importing and organizing...</div>
                                {:else}
                                    <h3>Import Complete!</h3>
                                    <p>Found and added {wizardImportResults.length} games.</p>
                                    <button class="btn-primary" onclick={() => { wizardOpen = false; if (selectedPlatform) selectPlatform(selectedPlatform); }}>Close</button>
                                {/if}
                            </div>
                        {/if}
                    </div>
                </div>
            </div>
        {/if}

        {#if currentView === 'library'}
            {#if selectedPlatform}
                <header class="view-header">
                    <div class="title-area">
                        <h1>{selectedPlatform.name}</h1>
                        <span class="count">{games.length} games</span>
                    </div>
                </header>
                <div class="game-grid">
                    {#each games as game}
                        <button class="game-card" onclick={() => scrapeGame(game)}>
                            <div class="thumbnail">
                                {#if thumbnails[game.id]}
                                    <img src={thumbnails[game.id]} alt={game.title} />
                                {:else}
                                    <div class="placeholder">
                                        <span>{game.title}</span>
                                        <small>Click to Scrape</small>
                                    </div>
                                {/if}
                            </div>
                            <div class="info">
                                <h3>{game.title}</h3>
                                <span class="platform">{selectedPlatform.name}</span>
                            </div>
                        </button>
                    {/each}
                </div>
            {:else}
                <div class="welcome-screen">
                    <div class="icon">📦</div>
                    <h1>Drag & Drop ROMs Here</h1>
                    <p>Drop any folder containing games to start the import wizard.</p>
                </div>
            {/if}
        {:else if currentView === 'emulators'}
            <div class="settings-view">
                <h1>Emulator Settings</h1>
                <p>Manage your emulators here.</p>
            </div>
        {/if}
    </main>
</div>

<style>
    :global(body) {
        margin: 0;
        padding: 0;
        background: #121212;
        color: #e0e0e0;
        font-family: 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
    }

    .app {
        display: flex;
        height: 100vh;
        overflow: hidden;
    }

    .sidebar {
        width: 280px;
        background: #181818;
        padding: 20px;
        border-right: 1px solid #282828;
        display: flex;
        flex-direction: column;
        gap: 20px;
        position: relative;
    }

    .sidebar .header {
        display: flex;
        align-items: center;
        gap: 15px;
    }

    .hamburger {
        background: none;
        border: none;
        cursor: pointer;
        display: flex;
        flex-direction: column;
        gap: 4px;
        padding: 5px;
    }

    .hamburger .bar {
        display: block;
        width: 20px;
        height: 2px;
        background: #fff;
        border-radius: 2px;
    }

    .menu-dropdown {
        position: absolute;
        top: 60px;
        left: 20px;
        right: 20px;
        background: #282828;
        border: 1px solid #383838;
        border-radius: 8px;
        box-shadow: 0 10px 25px rgba(0,0,0,0.5);
        z-index: 100;
        overflow: hidden;
    }

    .menu-dropdown button {
        width: 100%;
        padding: 12px 15px;
        background: none;
        border: none;
        color: #ddd;
        text-align: left;
        cursor: pointer;
        font-size: 0.9rem;
    }

    .menu-dropdown button:hover {
        background: #383838;
        color: #fff;
    }

    .sidebar button.btn-primary {
        background: #3b82f6;
        color: white;
        font-weight: 600;
        text-align: center;
        border: none;
        padding: 10px;
        border-radius: 6px;
        cursor: pointer;
    }

    .current-path {
        font-size: 0.7rem;
        color: #555;
        padding: 8px;
        background: #111;
        border-radius: 4px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .update-banner {
        background: #3b82f6;
        color: white;
        font-size: 0.75rem;
        padding: 10px;
        border-radius: 6px;
        text-align: center;
        animation: pulse 2s infinite;
    }

    @keyframes pulse {
        0% { opacity: 1; }
        50% { opacity: 0.7; }
        100% { opacity: 1; }
    }

    .platform-list {
        flex: 1;
        overflow-y: auto;
    }

    .sidebar ul {
        list-style: none;
        padding: 0;
        margin: 0;
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .sidebar li button {
        width: 100%;
        padding: 8px 12px;
        background: none;
        border: none;
        color: #aaa;
        text-align: left;
        cursor: pointer;
        border-radius: 4px;
        font-size: 0.9rem;
    }

    .sidebar li.active button {
        background: #3b82f6;
        color: white;
    }

    .content {
        flex: 1;
        padding: 30px;
        overflow-y: auto;
        background: #121212;
        position: relative;
    }

    .view-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 30px;
    }

    .view-header h1 {
        margin: 0;
        font-size: 2rem;
    }

    .game-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
        gap: 20px;
    }

    .game-card {
        background: #181818;
        border-radius: 8px;
        overflow: hidden;
        border: 1px solid #282828;
        transition: transform 0.2s;
        text-align: left;
        padding: 0;
        cursor: pointer;
        color: inherit;
        font-family: inherit;
    }

    .game-card:hover {
        transform: scale(1.03);
        border-color: #383838;
    }

    .thumbnail {
        aspect-ratio: 3/4;
        background: #222;
        display: flex;
        align-items: center;
        justify-content: center;
        text-align: center;
        padding: 10px;
        font-size: 0.8rem;
        color: #444;
    }

    .thumbnail img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }

    .placeholder {
        display: flex;
        flex-direction: column;
        padding: 15px;
    }

    .placeholder small {
        color: #3b82f6;
        margin-top: 5px;
    }

    .info {
        padding: 12px;
    }

    .info h3 {
        margin: 0;
        font-size: 0.85rem;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    /* Wizard Styles */
    .wizard-overlay {
        position: absolute;
        top: 0; left: 0; right: 0; bottom: 0;
        background: rgba(0,0,0,0.8);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        backdrop-filter: blur(10px);
    }

    .wizard-card {
        background: #1e1e1e;
        width: 500px;
        padding: 30px;
        border-radius: 12px;
        border: 1px solid #333;
        box-shadow: 0 20px 50px rgba(0,0,0,0.5);
    }

    .wizard-card header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 20px;
    }

    .wizard-card h2 { margin: 0; font-size: 1.5rem; }

    .btn-close {
        background: none; border: none; color: #666; font-size: 2rem; cursor: pointer;
    }

    .steps {
        display: flex;
        gap: 10px;
        margin-bottom: 30px;
    }

    .step {
        flex: 1;
        height: 4px;
        background: #333;
        border-radius: 2px;
        font-size: 0.7rem;
        padding-top: 10px;
        color: #555;
    }

    .step.active {
        background: #3b82f6;
        color: #3b82f6;
    }

    .step-content {
        min-height: 200px;
    }

    .step-content select {
        width: 100%;
        padding: 12px;
        background: #111;
        border: 1px solid #333;
        color: #fff;
        border-radius: 6px;
        margin: 20px 0;
    }

    .radio-group {
        display: flex;
        flex-direction: column;
        gap: 12px;
        margin: 20px 0;
    }

    .radio-group label {
        display: flex;
        align-items: center;
        gap: 10px;
        cursor: pointer;
    }

    .wizard-actions {
        display: flex;
        justify-content: space-between;
        margin-top: 20px;
    }

    .wizard-actions button {
        background: #3b82f6;
        color: white;
        border: none;
        padding: 10px 20px;
        border-radius: 6px;
        cursor: pointer;
        font-weight: 600;
    }

    .wizard-actions button.btn-back {
        background: #333;
    }

    .checkbox {
        display: flex;
        align-items: center;
        gap: 10px;
        margin: 20px 0;
        cursor: pointer;
    }

    .welcome-screen {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        opacity: 0.5;
        border: 2px dashed #333;
        margin: 20px;
        border-radius: 20px;
    }

    .welcome-screen .icon { font-size: 5rem; margin-bottom: 20px; }

    .path-picker {
        display: flex;
        gap: 10px;
        margin: 10px 0;
    }

    .path-picker input {
        flex: 1;
        background: #111;
        border: 1px solid #333;
        color: #fff;
        padding: 8px;
        border-radius: 4px;
    }
</style>
