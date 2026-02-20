<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";

    let platforms = $state([]);
    let selectedPlatform = $state(null);
    let games = $state([]);
    let loading = $state(false);
    const nasPath = "/home/ubuntubox/freenas/Emulation/Aaron Program Files (x86)/LaunchBox";
    const mockPath = "/home/ubuntubox/mock_launchbox";
    let launchboxRoot = $state(nasPath); 

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
        loading = true;
        try {
            games = await invoke("get_games_for_platform", { platformId: platform.id });
            // For each game, ensure we have a thumbnail
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
                // Predict source path based on LaunchBox convention
                const sourcePath = `${launchboxRoot}/Images/${game.platform_id}/Box - Front/${game.title}-01.${ext}`;
                
                const cachePath = await invoke("generate_thumbnail", {
                    sourcePath,
                    gameId: game.id,
                    cacheDir,
                    width: 300,
                    height: 400
                });

                // Convert absolute path to our custom protocol URL
                thumbnails[game.id] = `game-media://localhost${cachePath}`;
                return; // Success, stop trying extensions
            } catch (e) {
                // Silently try next extension if it was a "Source image not found" error
                if (typeof e === 'string' && e.includes("Source image not found")) {
                    continue;
                }
                console.warn(`Failed to generate thumbnail for ${game.title} with .${ext}`, e);
            }
        }
    }

    async function autoDetect() {
        loading = true;
        try {
            const detectedPath = await invoke("detect_launchbox");
            if (detectedPath) {
                launchboxRoot = detectedPath;
                console.log("Auto-detected LaunchBox at", detectedPath);
                await loadPlatforms();
            }
        } catch (e) {
            console.error("Auto-detection failed", e);
        } finally {
            loading = false;
        }
    }

    onMount(() => {
        autoDetect();
    });
</script>

<div class="app">
    <aside class="sidebar">
        <h2>Platforms</h2>
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
        <ul>
            {#each platforms as platform}
                <li class={selectedPlatform?.id === platform.id ? 'active' : ''}>
                    <button onclick={() => selectPlatform(platform)}>{platform.name}</button>
                </li>
            {/each}
        </ul>
    </aside>

    <main class="content">
        {#if selectedPlatform}
            <header>
                <h1>{selectedPlatform.name}</h1>
                <span class="count">{games.length} games</span>
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
                <h1>TurboLaunch</h1>
                <p>Select a platform to browse your library or scan the mock directory.</p>
                <div class="path-info">Current Path: <code>{launchboxRoot}</code></div>
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
        font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
    }

    .app {
        display: flex;
        height: 100vh;
        overflow: hidden;
    }

    .sidebar {
        width: 280px;
        background: #1e1e1e;
        padding: 24px;
        border-right: 1px solid #333;
        display: flex;
        flex-direction: column;
        gap: 20px;
    }

    .sidebar h2 {
        margin: 0;
        font-size: 1.2rem;
        color: #fff;
        text-transform: uppercase;
        letter-spacing: 1px;
    }

    .sidebar ul {
        list-style: none;
        padding: 0;
        margin: 0;
        display: flex;
        flex-direction: column;
        gap: 8px;
        overflow-y: auto;
    }

    .sidebar button {
        width: 100%;
        padding: 12px 16px;
        background: #2a2a2a;
        color: #bbb;
        border: 1px solid #333;
        text-align: left;
        cursor: pointer;
        border-radius: 6px;
        transition: all 0.2s;
        font-size: 0.95rem;
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
    }

    .sidebar button.btn-small {
        padding: 6px 8px;
        font-size: 0.8rem;
        text-align: center;
    }

    .sidebar button:hover:not(:disabled) {
        filter: brightness(1.1);
    }

    .current-path {
        font-size: 0.75rem;
        color: #666;
        padding: 8px;
        background: #151515;
        border-radius: 4px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        border: 1px solid #222;
    }

    .sidebar li.active button {
        background: #3b82f6;
        color: white;
        border-color: #3b82f6;
        box-shadow: 0 4px 12px rgba(59, 130, 246, 0.3);
    }

    .sidebar button:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .content {
        flex: 1;
        padding: 40px;
        overflow-y: auto;
        background: #121212;
    }

    header {
        display: flex;
        align-items: baseline;
        gap: 16px;
        margin-bottom: 32px;
    }

    header h1 {
        margin: 0;
        font-size: 2.5rem;
        font-weight: 700;
        color: #fff;
    }

    header .count {
        color: #666;
        font-size: 1.1rem;
    }

    .game-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
        gap: 24px;
    }

    .game-card {
        background: #1e1e1e;
        border-radius: 12px;
        overflow: hidden;
        border: 1px solid #2a2a2a;
        transition: transform 0.2s, box-shadow 0.2s;
        cursor: pointer;
    }

    .game-card:hover {
        transform: translateY(-4px);
        box-shadow: 0 12px 24px rgba(0,0,0,0.4);
        border-color: #444;
    }

    .thumbnail {
        aspect-ratio: 3/4;
        background: linear-gradient(135deg, #2a2a2a 0%, #1a1a1a 100%);
        display: flex;
        align-items: center;
        justify-content: center;
        text-align: center;
        color: #555;
        font-weight: 500;
        overflow: hidden;
    }

    .thumbnail img {
        width: 100%;
        height: 100%;
        object-fit: cover;
    }

    .info {
        padding: 16px;
    }

    .game-card h3 {
        margin: 0;
        font-size: 0.95rem;
        font-weight: 600;
        color: #fff;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .platform {
        display: block;
        margin-top: 4px;
        font-size: 0.8rem;
        color: #666;
        text-transform: uppercase;
    }

    .welcome {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        text-align: center;
        color: #666;
    }

    .welcome h1 {
        font-size: 4rem;
        margin: 0;
        background: linear-gradient(to right, #3b82f6, #8b5cf6);
        -webkit-background-clip: text;
        -webkit-text-fill-color: transparent;
    }

    .path-info {
        margin-top: 20px;
        font-family: monospace;
        background: #1a1a1a;
        padding: 8px 12px;
        border-radius: 4px;
    }
</style>
