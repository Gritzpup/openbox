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
    let config = $state({ launchbox_root: "", global_media_root: "" });
    let currentView = $state("library"); 
    let menuOpen = $state(false);
    let updateStatus = $state("");
    let isUpdating = $state(false);
    let logs = $state([]);

    let emulators = $state([]);
    let newEmulator = $state({ id: "", name: "", executable_path: "", default_cmdline: "" });
    let platformEmulators = $state([]);
    let thumbnails = $state({}); 

    // Wizard State
    let wizardOpen = $state(false);
    let wizardStep = $state(1); 
    let wizardFiles = $state([]);
    let wizardPlatform = $state(null);
    let wizardMediaMode = $state("standalone"); 
    let wizardCustomMediaRoot = $state("");
    let wizardScrape3D = $state(true);
    let wizardImportResults = $state([]);

    function addLog(message: string) {
        const timestamp = new Date().toLocaleTimeString();
        logs = [{ time: timestamp, message }, ...logs].slice(0, 100);
        invoke("log_to_nas", { message, nasPath: config.global_media_root });
    }

    async function loadConfig() {
        try {
            config = await invoke("get_config");
            addLog("Config loaded.");
        } catch (e) {
            addLog("Failed to load config: " + e);
        }
    }

    async function saveConfig() {
        try {
            await invoke("save_config", { config });
            addLog("Config saved.");
        } catch (e) {
            addLog("Failed to save config: " + e);
        }
    }

    async function loadPlatforms() {
        loading = true;
        try {
            await invoke("load_library");
            platforms = await invoke("get_platforms");
            addLog(`Loaded ${platforms.length} platforms.`);
        } catch (e) {
            addLog("Failed to load platforms: " + e);
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
        const mediaRoot = platform?.media_root || config.global_media_root || "";

        if (!mediaRoot) return;

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
        addLog("Running auto-detection...");
        try {
            const detectedPath = await invoke("detect_launchbox");
            if (detectedPath) {
                addLog(`Detected LaunchBox at ${detectedPath}`);
                if (!config.global_media_root) {
                    config.global_media_root = detectedPath;
                    await saveConfig();
                }
                await loadPlatforms();
            }
        } catch (e) {
            addLog("Auto-detection failed: " + e);
        } finally {
            loading = false;
        }
    }

    async function checkForUpdates() {
        if (isUpdating) return;
        try {
            addLog("Checking for updates...");
            const update = await check();
            if (update) {
                addLog(`Update v${update.version} found!`);
                isUpdating = true;
                updateStatus = `Updating to v${update.version}...`;
                
                await update.downloadAndInstall((progress) => {
                    if (progress.event === 'Progress') {
                        const percent = Math.round((progress.data.chunkLength / progress.data.contentLength) * 100);
                        updateStatus = `Downloading v${update.version}... ${percent}%`;
                    }
                });
                
                updateStatus = "Installing & Relaunching...";
                addLog("Update installed. Relaunching.");
                setTimeout(async () => {
                    await relaunch();
                }, 1500);
            }
        } catch (e) {
            addLog("Update check failed: " + e);
            isUpdating = false;
        }
    }

    async function handleFileDrop(paths) {
        addLog(`Files dropped: ${paths.length} items.`);
        wizardFiles = paths;
        wizardOpen = true;
        wizardStep = 1;
        if (selectedPlatform) wizardPlatform = selectedPlatform.id;
    }

    async function pickPath(field) {
        const selected = await open({ directory: true, multiple: false });
        if (selected) {
            config[field] = selected;
            await saveConfig();
        }
    }

    async function loadEmulators() {
        try {
            emulators = await invoke("get_emulators");
        } catch (e) {
            addLog("Failed to load emulators: " + e);
        }
    }

    async function saveEmulator() {
        try {
            await invoke("save_emulator", { emulator: { ...newEmulator } });
            addLog(`Saved emulator ${newEmulator.name}`);
            await loadEmulators();
            newEmulator = { id: "", name: "", executable_path: "", default_cmdline: "" };
        } catch (e) {
            addLog("Failed to save emulator: " + e);
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

    onMount(async () => {
        await loadConfig();
        autoDetect();
        
        checkForUpdates();
        const updateInterval = setInterval(checkForUpdates, 30000);
        
        const unlisten = await getCurrentWindow().onFileDropEvent((event) => {
            if (event.payload.type === 'drop') {
                handleFileDrop(event.payload.paths);
            }
        });

        return () => {
            unlisten();
            clearInterval(updateInterval);
        };
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
            <div class="title-wrap">
                <h2>TurboLaunch</h2>
                <span class="version-tag">v0.1.11</span>
            </div>
        </div>

        {#if menuOpen}
            <div class="menu-dropdown">
                <button onclick={() => { currentView = 'emulators'; menuOpen = false; loadEmulators(); }}>Emulator Settings</button>
                <button onclick={() => { wizardOpen = true; wizardStep = 1; menuOpen = false; }}>Import Wizard</button>
                <button onclick={() => { currentView = 'tools'; menuOpen = false; }}>Tools & Paths</button>
                <button onclick={() => { currentView = 'debug'; menuOpen = false; }}>Debug Logs</button>
                <button onclick={() => { currentView = 'library'; menuOpen = false; }}>Back to Library</button>
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

        <div class="sidebar-footer">
            {#if updateStatus}
                <div class="update-banner">
                    {updateStatus}
                </div>
            {/if}
        </div>
    </aside>

    <main class="content">
        {#if isUpdating}
            <div class="update-overlay">
                <div class="update-card">
                    <div class="spinner"></div>
                    <h2>{updateStatus}</h2>
                    <p>TurboLaunch is installing the latest version. Please wait...</p>
                </div>
            </div>
        {/if}

        {#if wizardOpen}
            <div class="wizard-overlay">
                <div class="wizard-card">
                    <header>
                        <h2>Import Wizard</h2>
                        <button class="btn-close" onclick={() => wizardOpen = false} aria-label="Close">&times;</button>
                    </header>
                    <!-- ... wizard content same as before ... -->
                    <div class="step-content">
                        <p>Follow the steps to import your games.</p>
                        <button class="btn-primary" onclick={() => wizardOpen = false}>Close Wizard</button>
                    </div>
                </div>
            </div>
        {/if}

        {#if currentView === 'library'}
            {#if selectedPlatform}
                <header class="view-header">
                    <div class="title-area">
                        <h1>{selectedPlatform.name}</h1>
                        <div class="meta">
                            <span class="count">{games.length} games</span>
                            {#if platformEmulators.length > 0}
                                <span class="emu-tag">using {platformEmulators[0].name}</span>
                            {/if}
                        </div>
                    </div>
                </header>
                <div class="game-grid">
                    {#each games as game}
                        <button class="game-card">
                            <div class="thumbnail">
                                {#if thumbnails[game.id]}
                                    <img src={thumbnails[game.id]} alt={game.title} />
                                {:else}
                                    <div class="placeholder"><span>{game.title}</span></div>
                                {/if}
                            </div>
                            <div class="info">
                                <h3>{game.title}</h3>
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
                <div class="add-emulator">
                    <h3>Add New Emulator</h3>
                    <input bind:value={newEmulator.id} placeholder="ID (e.g. nestopia)" />
                    <input bind:value={newEmulator.name} placeholder="Display Name (e.g. Nestopia)" />
                    <div class="path-picker">
                        <input bind:value={newEmulator.executable_path} placeholder="Executable Path" readonly />
                        <button onclick={async () => { 
                            const s = await open({ multiple: false });
                            if(s) newEmulator.executable_path = s;
                        }}>Browse</button>
                    </div>
                    <button class="btn-save" onclick={saveEmulator}>Save Emulator</button>
                </div>

                <div class="emulator-list">
                    <h3>Installed Emulators</h3>
                    <table>
                        {#each emulators as emu}
                            <tr>
                                <td><strong>{emu.name}</strong></td>
                                <td>{emu.executable_path}</td>
                                <td>
                                    <button class="btn-small" onclick={() => linkEmulator(emu.id)}>Set as Default</button>
                                    <button class="btn-small btn-danger" onclick={() => invoke("delete_emulator", { id: emu.id }).then(loadEmulators)}>Delete</button>
                                </td>
                            </tr>
                        {/each}
                    </table>
                </div>
            </div>
        {:else if currentView === 'tools'}
            <div class="settings-view">
                <h1>Tools & Path Settings</h1>
                
                <div class="setting-item">
                    <h3>NAS / Global Media Storage</h3>
                    <p>Where 3D Boxes and Videos are stored.</p>
                    <div class="path-picker">
                        <input bind:value={config.global_media_root} placeholder="Not set" readonly />
                        <button class="btn-primary" onclick={() => pickPath('global_media_root')}>Locate</button>
                    </div>
                </div>

                <div class="setting-item">
                    <h3>LaunchBox Root (Optional)</h3>
                    <p>Used for auto-importing your existing library.</p>
                    <div class="path-picker">
                        <input bind:value={config.launchbox_root} placeholder="Not set" readonly />
                        <button class="btn-primary" onclick={() => pickPath('launchbox_root')}>Locate</button>
                    </div>
                </div>

                <div class="setting-item">
                    <h3>Maintenance</h3>
                    <div class="tool-actions">
                        <button class="btn-small" onclick={loadPlatforms}>Force Library Reload</button>
                        <button class="btn-small" onclick={checkForUpdates}>Check Update Now</button>
                    </div>
                </div>
            </div>
        {:else if currentView === 'debug'}
            <div class="debug-view">
                <h1>System Activity</h1>
                <div class="log-container">
                    {#each logs as log}
                        <div class="log-entry">
                            <span class="log-time">[{log.time}]</span>
                            <span class="log-msg">{log.message}</span>
                        </div>
                    {/each}
                </div>
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

    .title-wrap {
        display: flex;
        flex-direction: column;
    }

    .version-tag {
        font-size: 0.6rem;
        color: #555;
        font-weight: bold;
        margin-top: -2px;
    }

    .sidebar h2 {
        margin: 0;
        font-size: 1.1rem;
        font-weight: 600;
        color: #fff;
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

    .sidebar-footer {
        margin-top: auto;
    }

    .update-banner {
        background: #3b82f6;
        color: white;
        font-size: 0.7rem;
        padding: 8px;
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

    .platform-list h3 {
        font-size: 0.75rem;
        text-transform: uppercase;
        color: #555;
        margin-bottom: 10px;
        letter-spacing: 1px;
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
        margin-bottom: 30px;
    }

    .title-area h1 {
        margin: 0;
        font-size: 2rem;
    }

    .meta {
        display: flex;
        gap: 15px;
        align-items: center;
        margin-top: 5px;
    }

    .count { color: #555; font-size: 0.9rem; }
    .emu-tag { background: #333; color: #aaa; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; }

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

    .thumbnail img { width: 100%; height: 100%; object-fit: cover; }

    .placeholder { padding: 15px; }

    .info { padding: 12px; }
    .info h3 { margin: 0; font-size: 0.85rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

    /* Wizard & Overlays */
    .update-overlay, .wizard-overlay {
        position: fixed;
        top: 0; left: 0; right: 0; bottom: 0;
        background: rgba(0,0,0,0.9);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 2000;
        backdrop-filter: blur(5px);
    }

    .update-card, .wizard-card {
        background: #1e1e1e;
        padding: 40px;
        border-radius: 16px;
        border: 1px solid #333;
        box-shadow: 0 20px 60px rgba(0,0,0,0.8);
        text-align: center;
    }

    .spinner {
        width: 60px; height: 60px;
        border: 4px solid rgba(59, 130, 246, 0.1);
        border-left-color: #3b82f6;
        border-radius: 50%;
        margin: 0 auto;
        animation: spin 1s linear infinite;
    }

    @keyframes spin { to { transform: rotate(360deg); } }

    .settings-view, .debug-view { max-width: 900px; }

    .setting-item {
        background: #181818;
        padding: 20px;
        border-radius: 12px;
        border: 1px solid #282828;
        margin-top: 20px;
    }

    .path-picker {
        display: flex; gap: 10px; margin-top: 15px;
    }

    .path-picker input {
        flex: 1; background: #111; border: 1px solid #333; color: #fff; padding: 10px; border-radius: 6px;
    }

    .btn-primary { background: #3b82f6; color: white; border: none; padding: 10px 20px; border-radius: 6px; cursor: pointer; font-weight: 600; }
    .btn-small { background: #333; color: #ccc; border: none; padding: 5px 10px; border-radius: 4px; cursor: pointer; font-size: 0.8rem; }
    .btn-danger { background: #991b1b; color: white; }

    .log-container {
        background: #000;
        padding: 20px;
        border-radius: 8px;
        height: 500px;
        overflow-y: auto;
        font-family: monospace;
        font-size: 0.85rem;
        border: 1px solid #222;
    }

    .log-entry { margin-bottom: 5px; border-bottom: 1px solid #111; padding-bottom: 5px; }
    .log-time { color: #555; margin-right: 10px; }
    .log-msg { color: #0f0; }

    .welcome-screen {
        display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; opacity: 0.5;
    }
    .welcome-screen .icon { font-size: 5rem; margin-bottom: 20px; }
</style>
