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
    let config = $state({ data_root: null, launchbox_root: "", global_media_root: "" });
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

    // Wizard/Setup State
    let setupWizardOpen = $state(false);
    let importWizardOpen = $state(false);
    let emulatorSetupOpen = $state(false);
    let wizardStep = $state(1); 
    let wizardFiles = $state([]);
    let wizardPlatform = $state("");
    let wizardEmulator = $state(null);
    let wizardFileAction = $state("link"); // link, copy, move
    let wizardSearchMetadata = $state(true);
    let wizardMediaOptions = $state({
        box_3d: true,
        box_front: true,
        box_back: false,
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
        fanart: true,
        disc: true
    });
    let wizardImportResults = $state([]);
    let installingStatus = $state("");

    const STANDARD_PLATFORMS = [
        "Sony PlayStation", "Sony PlayStation 2", "Sony PlayStation 3", "Sony PlayStation Portable",
        "Nintendo Entertainment System", "Super Nintendo Entertainment System", "Nintendo 64", 
        "Nintendo GameCube", "Nintendo Wii", "Nintendo Wii U", "Nintendo Switch",
        "Nintendo Game Boy", "Nintendo Game Boy Color", "Nintendo Game Boy Advance", "Nintendo DS", "Nintendo 3DS",
        "Sega Genesis", "Sega Saturn", "Sega Dreamcast", "Sega Master System", "Sega Game Gear",
        "Arcade", "MAME", "SNK Neo Geo AES", "Atari 2600", "Atari 5200", "Atari 7800", "PC"
    ];

    const CURRENT_VERSION = "v0.1.60";

    function addLog(message: string) {
        const timestamp = new Date().toLocaleTimeString();
        logs = [{ time: timestamp, message }, ...logs].slice(0, 100);
        console.log(`[JS LOG] ${message}`);
        if (config.data_root) {
            invoke("log_to_nas", { message, nas_path: config.data_root });
        }
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
            await invoke("load_library");
            platforms = await invoke("get_platforms");
            addLog(`Loaded ${platforms.length} platforms.`);
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
        try {
            games = await invoke("get_games_for_platform", { platformId: platform.id });
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

    async function checkForUpdates() {
        if (isUpdating || isChecking) return;
        isChecking = true;
        updateStatus = "Checking...";
        const checkStartTime = Date.now();
        
        try {
            addLog("Update engine: Starting check...");
            if (config.data_root) {
                invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: null });
            }
            
            const update = await check();
            lastChecked = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
            
            if (update) {
                addLog(`Update engine: Found v${update.version}!`);
                updateError = ""; 
                isUpdating = true;
                updateStatus = `Downloading v${update.version}...`;
                
                try {
                    await update.downloadAndInstall((progress) => {
                        if (progress.event === 'Progress') {
                            const percent = progress.data.contentLength ? Math.round((progress.data.chunkLength / progress.data.contentLength) * 100) : 0;
                            updateStatus = `Downloading... ${percent}%`;
                        }
                    });
                    
                    updateStatus = "Restarting...";
                    setTimeout(async () => { await relaunch(); }, 300);
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
        addLog(`[File Drop] Detected ${paths.length} items: ${paths.join(', ')}`);
        wizardFiles = paths;
        importWizardOpen = true;
        wizardStep = 1;
        await loadEmulators();
        // Try to auto-detect platform from files? For now just reset
        wizardPlatform = "";
    }

    async function runWizardImport() {
        loading = true;
        addLog(`Starting Import for platform: ${wizardPlatform} (${wizardFiles.length} files)`);
        try {
            // 1. Scaffold directories on NAS
            if (config.data_root) {
                await invoke("scaffold_platform_directories", { 
                    masterPath: config.data_root, 
                    platformId: wizardPlatform 
                });
            }

            // 2. Link Emulator if selected
            if (wizardEmulator) {
                await invoke("link_platform_emulator", {
                    platformId: wizardPlatform,
                    emulatorId: wizardEmulator,
                    isDefault: true
                });
            }

            // 3. Perform Batch Import with File Action
            const results = [];
            for (const path of wizardFiles) {
                const res = await invoke("batch_import", {
                    folderPath: path,
                    platformId: wizardPlatform,
                    fileAction: wizardFileAction, // New parameter
                    mediaRoot: null 
                });
                
                // 4. Media Scraping (Simple pass-through for now, can be expanded)
                for (const title of res) {
                    if (wizardMediaOptions.box_3d || wizardMediaOptions.gameplay_video) {
                        try {
                            const scraped = await invoke("scrape_game_art", { platform: wizardPlatform, title });
                            const mediaRoot = config.data_root || config.global_media_root;
                            
                            if (wizardMediaOptions.box_3d && scraped.box_3d_url) {
                                const dest = `${mediaRoot}/Images/${wizardPlatform}/Box - 3D/${title}-01.png`;
                                invoke("download_art", { url: scraped.box_3d_url, destinationPath: dest });
                            }
                            if (wizardMediaOptions.gameplay_video && scraped.video_url) {
                                const dest = `${mediaRoot}/Videos/${wizardPlatform}/${title}-01.mp4`;
                                invoke("download_art", { url: scraped.video_url, destinationPath: dest });
                            }
                        } catch (scrapeErr) {
                            addLog(`Download failed for ${title}: ${scrapeErr}`);
                        }
                    }
                }
                results.push(...res);
            }
            wizardImportResults = results;
            wizardStep = 5; // Success!
            await loadPlatforms();
        } catch (e) {
            addLog("Import failed: " + e);
            alert("Import failed: " + e);
        } finally {
            loading = false;
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

    async function resetUpdateState() {
        isUpdating = false; isChecking = false;
        updateStatus = "";
        lastChecked = "Resetting...";
        addLog("Manual Update Reset triggered.");
        checkForUpdates();
    }

    onMount(() => {
        addLog("App mounting...");
        
        // Start updates immediately
        checkForUpdates();
        const updateInterval = setInterval(checkForUpdates, 30000);

        // Async background tasks
        (async () => {
            try {
                const unlisten = await getCurrentWindow().onDragDropEvent((event) => {
                    if (event.payload.type === 'drop') {
                        isDragging = false;
                        handleFileDrop(event.payload.paths);
                    } else if (event.payload.type === 'enter' || event.payload.type === 'over') {
                        isDragging = true;
                    } else {
                        isDragging = false;
                    }
                });
                
                await loadConfig();
                if (config.data_root) {
                    await loadPlatforms();
                    invoke("report_version", { version: CURRENT_VERSION, nas_path: config.data_root, error: null });
                }
            } catch (err) {
                addLog(`Startup background error: ${err}`);
            }
        })();

        return () => {
            clearInterval(updateInterval);
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
            {#if updateError}
                <div class="update-error-msg" title={updateError}>
                    ⚠️ Update Failed
                    <button class="btn-tiny" onclick={resetUpdateState}>Retry</button>
                </div>
            {/if}
            <div class="update-status-minimal">
                <div class="mini-spinner" class:rotating={isUpdating || isChecking}></div>
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
                    <header>
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
                        {#if wizardStep === 1}
                            <div class="step-inner">
                                <h3>What platform are you importing games for?</h3>
                                <p>Select the platform that these games belong to.</p>
                                <div class="selection-box">
                                    <select bind:value={wizardPlatform} size="10">
                                        {#each STANDARD_PLATFORMS as p}
                                            <option value={p}>{p}</option>
                                        {/each}
                                    </select>
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
                                <div class="or-divider">OR</div>
                                <button class="btn-retroarch" onclick={runAutoEmulatorSetup}>🚀 Auto-Setup Recommended Emulators</button>
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
                                <div class="metadata-toggle">
                                    <label class="checkbox-large">
                                        <input type="checkbox" bind:checked={wizardSearchMetadata} />
                                        <span>Search for game information in local metadata database</span>
                                    </label>
                                </div>
                                
                                <p>How would you like to download images for your games?</p>
                                <div class="media-grid-scroll">
                                    <div class="media-column">
                                        <h4>Marketing & Box Art</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_3d} /> Box - 3D</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_front} /> Box - Front</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.box_back} /> Box - Back</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.flyer_front} /> Advertisement Flyer - Front</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.flyer_back} /> Advertisement Flyer - Back</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.banner} /> Banner</label>
                                    </div>
                                    <div class="media-column">
                                        <h4>Arcade Specific</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_cabinet} /> Arcade - Cabinet</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_marquee} /> Arcade - Marquee</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_board} /> Arcade - Circuit Board</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_control_panel} /> Arcade - Control Panel</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.arcade_controls_info} /> Arcade - Controls Info</label>
                                    </div>
                                    <div class="media-column">
                                        <h4>Video & In-Game</h4>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.bigbox_video} /> Big Box Cinematic Video</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.gameplay_video} /> Gameplay Video</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.clear_logo} /> Clear Logo</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.fanart} /> Fanart - Background</label>
                                        <label class="checkbox"><input type="checkbox" bind:checked={wizardMediaOptions.disc} /> Disc / Cartridge Media</label>
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
                <header class="view-header">
                    <h1>{selectedPlatform.name} <span class="count">({games.length})</span></h1>
                    {#if platformEmulators.length > 0}
                        <span class="emu-tag">Default: {platformEmulators[0].name}</span>
                    {/if}
                </header>
                <div class="game-grid">
                    {#each games as game}
                        <div class="game-card">
                            <div class="thumbnail" ondblclick={() => playGame(game.id)}>
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
                            </div>
                        </div>
                    {/each}
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
</div>

<style>
    :global(body) { margin: 0; padding: 0; background: #121212; color: #e0e0e0; font-family: 'Segoe UI', system-ui, sans-serif; }
    .app { display: flex; height: 100vh; overflow: hidden; position: relative; }
    .sidebar { width: 280px; background: #181818; padding: 20px; border-right: 1px solid #282828; display: flex; flex-direction: column; gap: 20px; position: relative; }
    .sidebar .header { display: flex; align-items: center; gap: 15px; }
    .hamburger { background: none; border: none; cursor: pointer; display: flex; flex-direction: column; gap: 4px; }
    .hamburger .bar { display: block; width: 20px; height: 2px; background: #fff; border-radius: 2px; }
    .title-wrap { display: flex; flex-direction: column; }
    .version-tag { font-size: 0.6rem; color: #555; font-weight: bold; margin-top: -2px; }
    .sidebar h2 { margin: 0; font-size: 1.1rem; font-weight: 600; color: #fff; }
    .menu-dropdown { position: absolute; top: 60px; left: 20px; right: 20px; background: #282828; border: 1px solid #383838; border-radius: 8px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); z-index: 100; overflow: hidden; }
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

    .mini-spinner { width: 10px; height: 10px; border: 2px solid rgba(255, 255, 255, 0.05); border-left-color: #333; border-radius: 50%; }
    .mini-spinner.rotating { border-left-color: #3b82f6; animation: spin 1s linear infinite; }
    .update-info { display: flex; flex-direction: column; }
    .status-msg { color: #3b82f6; font-weight: bold; }
    @keyframes spin { to { transform: rotate(360deg); } }
    .platform-list { flex: 1; overflow-y: auto; }
    .platform-list h3 { font-size: 0.75rem; text-transform: uppercase; color: #555; margin-bottom: 10px; }
    .sidebar ul { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 4px; }
    .sidebar li button { width: 100%; padding: 8px 12px; background: none; border: none; color: #aaa; text-align: left; cursor: pointer; border-radius: 4px; }
    .sidebar li.active button { background: #3b82f6; color: white; }
    .content { flex: 1; padding: 30px; overflow-y: auto; background: #121212; position: relative; }
    .game-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 20px; }
    .game-card { background: #181818; border-radius: 8px; overflow: hidden; border: 1px solid #282828; text-align: left; padding: 0; cursor: pointer; color: inherit; }
    .thumbnail { aspect-ratio: 3/4; background: #222; display: flex; align-items: center; justify-content: center; text-align: center; position: relative; }
    .thumbnail img { width: 100%; height: 100%; object-fit: cover; }
    .card-overlay { position: absolute; top: 0; left: 0; right: 0; bottom: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; opacity: 0; transition: opacity 0.2s; }
    .game-card:hover .card-overlay { opacity: 1; }
    .btn-play-icon { width: 50px; height: 50px; border-radius: 50%; background: #3b82f6; color: white; border: none; font-size: 1.5rem; cursor: pointer; display: flex; align-items: center; justify-content: center; padding-left: 5px; }
    .info { padding: 12px; }
    .info h3 { margin: 0; font-size: 0.85rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
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
    .metadata-toggle { margin-bottom: 30px; background: #181818; padding: 20px; border-radius: 12px; border: 1px solid #333; }
    .checkbox-large { display: flex; align-items: center; gap: 15px; font-weight: bold; color: #fff; cursor: pointer; }
    .checkbox-large input { width: 20px; height: 20px; }

    .wizard-actions { margin-top: 40px; display: flex; justify-content: flex-end; gap: 15px; border-top: 1px solid #333; padding-top: 25px; }
    .btn-secondary { background: #333; color: #eee; border: none; padding: 10px 25px; border-radius: 6px; cursor: pointer; font-weight: 600; }
    .btn-secondary:hover { background: #444; }

    .success-step { text-align: center; }
    .icon-big { font-size: 5rem; margin-bottom: 20px; }
    .import-summary { background: #000; padding: 20px; border-radius: 8px; font-family: monospace; font-size: 0.8rem; text-align: left; max-height: 200px; overflow-y: auto; color: #0f0; border: 1px solid #222; }
    .summary-item { margin-bottom: 4px; }
    .summary-more { color: #888; margin-top: 10px; font-style: italic; }
</style>
