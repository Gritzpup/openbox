<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { open } from "@tauri-apps/plugin-dialog";

    let platforms = $state([]);
    let selectedPlatform = $state(null);
    let games = $state([]);
    let loading = $state(false);
    const nasPath = "/home/ubuntubox/freenas/Emulation/Aaron Program Files (x86)/LaunchBox";
    const mockPath = "/home/ubuntubox/mock_launchbox";
    let launchboxRoot = $state(nasPath); 
    let currentView = $state("library"); // main, emulators, platforms, tools
    let menuOpen = $state(false);

    let emulators = $state([]);
    let newEmulator = $state({ id: "", name: "", executable_path: "", default_cmdline: "" });

    async function startScanNas() {
        launchboxRoot = nasPath;
        await startScan();
    }

    async function startScanLocal() {
        launchboxRoot = mockPath;
        await startScan();
    }

    async function startScan() {
        loading = true;
        try {
            await invoke("start_scan", { launchboxRoot });
            await loadPlatforms();
        } catch (e) {
            console.error("Scan failed", e);
        } finally {
            loading = false;
        }
    }
    let thumbnails = $state({}); // Map of game_id -> thumbnail_url

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
        const extensions = ["png", "jpg", "jpeg"];
        const cacheDir = `${launchboxRoot}/Cache/Thumbnails`;
        for (const ext of extensions) {
            try {
                const sourcePath = `${launchboxRoot}/Images/${game.platform_id}/Box - Front/${game.title}-01.${ext}`;
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
                if (typeof e === 'string' && e.includes("Source image not found")) continue;
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

    // Settings logic
    async function loadEmulators() {
        try {
            emulators = await invoke("get_emulators");
        } catch (e) {
            console.error("Failed to load emulators", e);
        }
    }

    async function saveEmulator() {
        try {
            await invoke("save_emulator", { emulator: { ...newEmulator } });
            await loadEmulators();
            newEmulator = { id: "", name: "", executable_path: "", default_cmdline: "" };
        } catch (e) {
            console.error("Failed to save emulator", e);
        }
    }

    async function pickEmulatorPath() {
        const selected = await open({
            multiple: false,
            filters: [{ name: 'Executable', extensions: ['exe', 'bin', 'sh'] }]
        });
        if (selected) {
            newEmulator.executable_path = selected;
        }
    }

    async function handleAddGame() {
        const selected = await open({
            multiple: false,
            filters: [{ name: 'Game Rom', extensions: ['zip', 'nes', 'smc', 'sfc', 'iso'] }]
        });
        if (selected) {
            const title = selected.split('/').pop().split('.').shift();
            const id = Math.random().toString(36).substring(7);
            try {
                await invoke("add_game", {
                    id,
                    platform_id: selectedPlatform?.id || "Unknown",
                    title,
                    filePath: selected
                });
                if (selectedPlatform) selectPlatform(selectedPlatform);
            } catch (e) {
                console.error("Failed to add game", e);
            }
        }
    }

    onMount(() => {
        autoDetect();
    });
</script>

<div class="app">
    <aside class="sidebar">
        <div class="header">
            <button class="hamburger" onclick={() => menuOpen = !menuOpen}>
                <span class="bar"></span>
                <span class="bar"></span>
                <span class="bar"></span>
            </button>
            <h2>TurboLaunch</h2>
        </div>

        {#if menuOpen}
            <div class="menu-dropdown">
                <button onclick={() => { currentView = 'emulators'; menuOpen = false; loadEmulators(); }}>Emulator Settings</button>
                <button onclick={() => { currentView = 'platforms'; menuOpen = false; }}>Platform Settings</button>
                <button onclick={() => { currentView = 'tools'; menuOpen = false; }}>Tools</button>
                <button onclick={() => { currentView = 'library'; menuOpen = false; }}>Back to Library</button>
            </div>
        {/if}

        <div class="actions">
            <button onclick={autoDetect} disabled={loading} class="btn-primary">
                {loading ? 'Detecting...' : 'Auto-Detect'}
            </button>
            <div class="manual-scans">
                <button onclick={startScanLocal} disabled={loading} class="btn-small">Scan Mock</button>
                <button onclick={startScanNas} disabled={loading} class="btn-small">Scan NAS</button>
            </div>
        </div>

        <div class="current-path" title={launchboxRoot}>
            Path: {launchboxRoot.split('/').pop()}
        </div>

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
        {#if currentView === 'library'}
            {#if selectedPlatform}
                <header class="view-header">
                    <div class="title-area">
                        <h1>{selectedPlatform.name}</h1>
                        <span class="count">{games.length} games</span>
                    </div>
                    <button class="btn-add" onclick={handleAddGame}>+ Add Game</button>
                </header>
                <div class="game-grid">
                    {#each games as game}
                        <div class="game-card">
                            <div class="thumbnail">
                                {#if thumbnails[game.id]}
                                    <img src={thumbnails[game.id]} alt={game.title} />
                                {:else}
                                    <span>{game.title}</span>
                                {/if}
                            </div>
                            <div class="info">
                                <h3>{game.title}</h3>
                                <span class="platform">{selectedPlatform.name}</span>
                            </div>
                        </div>
                    {/each}
                </div>
            {:else}
                <div class="welcome">
                    <h1>Welcome</h1>
                    <p>Select a platform to start browsing.</p>
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
                        <button onclick={pickEmulatorPath}>Browse</button>
                    </div>
                    <input bind:value={newEmulator.default_cmdline} placeholder="Default Command Line Args" />
                    <button class="btn-save" onclick={saveEmulator}>Save Emulator</button>
                </div>

                <div class="emulator-list">
                    <h3>Installed Emulators</h3>
                    <table>
                        <thead>
                            <tr>
                                <th>Name</th>
                                <th>Path</th>
                                <th>Actions</th>
                            </tr>
                        </thead>
                        <tbody>
                            {#each emulators as emu}
                                <tr>
                                    <td>{emu.name}</td>
                                    <td>{emu.executable_path}</td>
                                    <td>
                                        <button onclick={() => invoke("delete_emulator", { id: emu.id }).then(loadEmulators)}>Delete</button>
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            </div>
        {:else if currentView === 'platforms'}
            <div class="settings-view">
                <h1>Platform Settings</h1>
                <p>Platform management coming soon...</p>
            </div>
        {:else if currentView === 'tools'}
            <div class="settings-view">
                <h1>Tools</h1>
                <div class="tool-actions">
                    <button onclick={startScan}>Full Library Re-scan</button>
                    <button onclick={() => thumbnails = {}}>Clear Image Cache</button>
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

    .actions {
        display: flex;
        flex-direction: column;
        gap: 10px;
    }

    .manual-scans {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 8px;
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

    .sidebar button.btn-small {
        padding: 6px 8px;
        font-size: 0.75rem;
        background: #282828;
        border: 1px solid #383838;
        color: #aaa;
        border-radius: 4px;
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

    .count {
        color: #555;
        font-size: 0.9rem;
    }

    .btn-add {
        background: #10b981;
        color: white;
        border: none;
        padding: 8px 16px;
        border-radius: 6px;
        cursor: pointer;
        font-weight: 600;
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

    .settings-view {
        max-width: 800px;
    }

    .add-emulator {
        background: #181818;
        padding: 20px;
        border-radius: 8px;
        margin-bottom: 30px;
        display: flex;
        flex-direction: column;
        gap: 12px;
    }

    .add-emulator input {
        background: #111;
        border: 1px solid #333;
        color: #fff;
        padding: 10px;
        border-radius: 4px;
    }

    .path-picker {
        display: flex;
        gap: 10px;
    }

    .path-picker input {
        flex: 1;
    }

    .btn-save {
        background: #3b82f6;
        color: white;
        border: none;
        padding: 10px;
        border-radius: 4px;
        cursor: pointer;
    }

    table {
        width: 100%;
        border-collapse: collapse;
        margin-top: 20px;
    }

    th, td {
        text-align: left;
        padding: 12px;
        border-bottom: 1px solid #282828;
    }

    th {
        color: #555;
        font-size: 0.8rem;
        text-transform: uppercase;
    }

    .welcome {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        opacity: 0.3;
    }
</style>
