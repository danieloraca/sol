const invoke = window.__TAURI__?.core?.invoke;

const heroEl = document.querySelector("#hero");
const playerStageEl = document.querySelector("#player-stage");
const playerDetailsEl = document.querySelector("#player-details");
const trendingEl = document.querySelector("#trending");
const continueWatchingEl = document.querySelector("#continue-watching");
const catalogEl = document.querySelector("#catalog");
const streamsEl = document.querySelector("#streams");
const searchEl = document.querySelector("#search");
const searchButtonEl = document.querySelector("#search-button");
const searchFeedbackEl = document.querySelector("#search-feedback");
const mainViewEl = document.querySelector("#main-view");
const searchViewEl = document.querySelector("#search-view");
const searchBackEl = document.querySelector("#search-back");
const playerViewEl = document.querySelector("#player-view");
const playerBackEl = document.querySelector("#player-back");
const searchResultsEl = document.querySelector("#search-results");
const searchResultsTitleEl = document.querySelector("#search-results-title");
const searchResultsSummaryEl = document.querySelector("#search-results-summary");
const addonUrlEl = document.querySelector("#addon-url");
const installAddonButtonEl = document.querySelector("#install-addon");
const addonFeedbackEl = document.querySelector("#addon-feedback");
const addonsListEl = document.querySelector("#addons-list");
const addonDetailsEl = document.querySelector("#addon-details");
const settingsToggleEl = document.querySelector("#settings-toggle");
const settingsViewEl = document.querySelector("#settings-view");
const settingsBackEl = document.querySelector("#settings-back");
const filterButtons = [...document.querySelectorAll(".filter")];
const TORBOX_AUTO_REFRESH_INTERVAL_MS = 5000;
const TORBOX_AUTO_REFRESH_MAX_ATTEMPTS = 24;
const PLAYBACK_START_TIMEOUT_MS = 4000;

let activeFilter = "";
let itemCache = new Map();
let catalogItemsCache = [];
let homeFeed = null;
let selectedItemId = null;
let selectedStreams = [];
let selectedLookup = null;
let selectedStreamIndex = 0;
let selectedStreamProviderFilter = "all";
let playbackPercent = 0;
let isPlaying = false;
let isPlaybackStarting = false;
let playbackCurrentSeconds = 0;
let playbackDurationSeconds = 0;
let pendingSeekSeconds = null;
let lastPlaybackError = "";
let lastPlaybackNotice = "";
let lastPlaybackNoticeKind = "";
let playbackStartTimer = null;
let torboxSubmissionState = null;
let torboxDraftMagnet = "";
let torboxCachedOnly = true;
let torboxAutoRefreshTimer = null;
let torboxAutoRefreshAttempt = 0;
let installedAddons = [];
let selectedAddonSource = null;
let autoPlayPending = false;
let manualSourceToolsVisible = false;
let autoPlayTrace = null;
let playbackActivated = false;
let lastExecutedSearch = "";
let isSearchViewActive = false;
let currentPage = "main";
let playerReturnPage = "main";
let selectItemRequestToken = 0;
let fullscreenListenerBound = false;
let isPlayerFullscreen = false;
let isNativeFullscreen = false;
let fullscreenControlsTimer = null;
let fullscreenPointerTicking = false;
let lastFullscreenControlsRefreshMs = 0;

async function bootstrap() {
  if (!invoke) {
    renderShellError("Tauri runtime not detected. Launch this through `cargo run` to use the desktop shell.");
    return;
  }

  await Promise.all([renderHome(), renderCatalog()]);
  window.requestAnimationFrame(() => {
    void renderAddons().catch((error) => {
      addonFeedbackEl.textContent = String(error);
    });
  });
  window.requestAnimationFrame(() => {
    if (homeFeed?.hero?.id) {
      void selectItem(homeFeed.hero.id);
    }
  });

  searchEl.addEventListener("input", handleSearch);
  searchEl.addEventListener("keydown", async (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      await runSearch();
    }
  });
  searchButtonEl?.addEventListener("click", async () => {
    await runSearch();
  });
  searchBackEl?.addEventListener("click", () => {
    showMainView();
  });
  playerBackEl?.addEventListener("click", () => {
    if (playerReturnPage === "search") {
      showSearchView();
      return;
    }
    showMainView();
  });
  filterButtons.forEach((button) => {
    button.addEventListener("click", async () => {
      activeFilter = button.dataset.filter ?? "";
      filterButtons.forEach((item) => item.classList.toggle("is-active", item === button));
      searchEl.value = "";
      lastExecutedSearch = "";
      setSearchFeedback("");
      showMainView();
      await renderCatalog();
    });
  });

  installAddonButtonEl?.addEventListener("click", async () => {
    const manifestUrl = addonUrlEl?.value?.trim() ?? "";
    if (!manifestUrl) {
      addonFeedbackEl.textContent = "Paste a manifest URL first.";
      return;
    }

    addonFeedbackEl.textContent = "Installing addon...";

    try {
      const descriptor = await invoke("install_addon_url", { manifestUrl });
      addonFeedbackEl.textContent = `Installed ${descriptor.name}. Reloading catalog...`;
      addonUrlEl.value = "";
      await reloadAddonDrivenViews();
      addonFeedbackEl.textContent = `Installed ${descriptor.name}.`;
    } catch (error) {
      addonFeedbackEl.textContent = String(error);
    }
  });

  settingsToggleEl?.addEventListener("click", () => {
    toggleSettingsModal();
  });
  settingsBackEl?.addEventListener("click", () => {
    closeSettingsModal();
  });
  settingsViewEl?.addEventListener("click", (event) => {
    if (event.target === settingsViewEl) {
      closeSettingsModal();
    }
  });

  if (!fullscreenListenerBound) {
    document.addEventListener("mousemove", () => {
      if (!isPlayerFullscreen || fullscreenPointerTicking) {
        return;
      }

      fullscreenPointerTicking = true;
      window.requestAnimationFrame(() => {
        fullscreenPointerTicking = false;
        showFullscreenControls();
      });
    });
    document.addEventListener("touchstart", () => {
      if (isPlayerFullscreen) {
        showFullscreenControls();
      }
    }, { passive: true });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && isPlayerFullscreen) {
        setPlayerFullscreen(false);
        return;
      }
      if (event.key === "Escape" && isSettingsModalOpen()) {
        closeSettingsModal();
      }
    });

    document.addEventListener("fullscreenchange", () => {
      const item = currentItem();
      const stream = activeStreamForSelection();
      if (item && stream) {
        syncPlayerUi(item, stream);
      }
    });
    fullscreenListenerBound = true;
  }
}

async function renderHome() {
  homeFeed = await invoke("get_home_feed");
  cacheItems([homeFeed.hero, ...homeFeed.trending, ...homeFeed.continue_watching]);

  heroEl.innerHTML = `
    <div class="hero-media ${heroArtworkUrl(homeFeed.hero) ? "" : "is-fallback"}">
      ${renderArtworkImage(homeFeed.hero, "hero-poster")}
      <div class="hero-copy">
        <p class="eyebrow">Featured</p>
        <h2>${homeFeed.hero.title}</h2>
        <p>${homeFeed.hero.description}</p>
        <p class="meta">${homeFeed.hero.year} • ${homeFeed.hero.media_type} • ${homeFeed.hero.genres.join(" / ")}</p>
        <div class="hero-actions">
          <button class="primary-button" data-play-hero="${homeFeed.hero.id}">Play featured</button>
          <button class="ghost-button" data-open-hero="${homeFeed.hero.id}">Open player</button>
        </div>
      </div>
    </div>
  `;

  trendingEl.innerHTML = homeFeed.trending.map(renderCard).join("");
  continueWatchingEl.innerHTML = homeFeed.continue_watching.map(renderCard).join("");
  bindCatalogButtons(trendingEl);
  bindCatalogButtons(continueWatchingEl);
  bindHeroButtons();
}

async function renderCatalog() {
  const items = await invoke("get_catalog", {
    mediaType: activeFilter || null,
  });

  catalogItemsCache = items;
  cacheItems(items);
  if (items.length === 0) {
    renderCatalogEmpty("No titles available for this filter yet.");
    return;
  }

  catalogEl.innerHTML = items.map(renderCard).join("");
  bindCatalogButtons(catalogEl);
}

async function renderAddons() {
  installedAddons = await invoke("get_addons");
  if (!selectedAddonSource || !installedAddons.some((addon) => addon.source === selectedAddonSource)) {
    selectedAddonSource = installedAddons[0]?.source ?? null;
  }
  addonsListEl.innerHTML = installedAddons
    .map(
      (addon) => `
        <article class="addon-card ${addon.source === selectedAddonSource ? "is-active" : ""}" data-addon-select="${escapeHtml(addon.source)}">
          <div class="addon-card-copy">
            <h3>${escapeHtml(addon.name)}</h3>
            <p class="meta">${escapeHtml(addon.id)} • ${escapeHtml(addon.transport)}</p>
            <p class="meta">${escapeHtml(addon.capabilities.join(" / "))}</p>
          </div>
          <div class="addon-card-meta">
            <span class="provider-badge ${addon.enabled ? "is-success" : "is-neutral"}">
              ${addon.enabled ? "Enabled" : "Disabled"}
            </span>
            <span class="provider-badge ${addon.configured ? "is-success" : "is-error"}">
              ${addon.configured ? "Configured" : "Needs setup"}
            </span>
            <span class="provider-badge ${addonHealthClass(addon)}">
              ${escapeHtml(addonHealthLabel(addon))}
            </span>
          </div>
          <p class="meta">${escapeHtml(addon.health_message)}</p>
          ${
            addon.transport === "remote"
              ? `
                <div class="addon-actions">
                  <button class="ghost-button addon-action" data-addon-move="up" data-addon-source="${escapeHtml(addon.source)}">Up</button>
                  <button class="ghost-button addon-action" data-addon-move="down" data-addon-source="${escapeHtml(addon.source)}">Down</button>
                  <button class="ghost-button addon-action" data-addon-toggle="${escapeHtml(addon.source)}" data-addon-enabled="${addon.enabled}">
                    ${addon.enabled ? "Disable" : "Enable"}
                  </button>
                  <button class="ghost-button addon-action is-danger" data-addon-remove="${escapeHtml(addon.source)}">Remove</button>
                </div>
              `
              : `<p class="meta">Built-in addon</p>`
          }
        </article>
      `,
    )
    .join("");

  renderAddonDetails();
  bindAddonActions();
}

function bindAddonActions() {
  addonsListEl.querySelectorAll("[data-addon-select]").forEach((card) => {
    card.addEventListener("click", () => {
      selectedAddonSource = card.dataset.addonSelect;
      renderAddons();
    });
  });

  bindAddonManagementActions(addonsListEl);
  bindAddonManagementActions(addonDetailsEl);

  addonDetailsEl.querySelectorAll("[data-addon-open]").forEach((button) => {
    button.addEventListener("click", async () => {
      const url = button.dataset.addonOpen;
      addonFeedbackEl.textContent = "Opening addon manifest...";

      try {
        await invoke("open_external_url", { url });
        addonFeedbackEl.textContent = "Opened addon manifest.";
      } catch (error) {
        addonFeedbackEl.textContent = String(error);
      }
    });
  });
}

function bindAddonManagementActions(scope) {
  scope.querySelectorAll("[data-addon-toggle]").forEach((button) => {
    button.addEventListener("click", async () => {
      const manifestUrl = button.dataset.addonToggle;
      const enabled = button.dataset.addonEnabled !== "true";
      addonFeedbackEl.textContent = `${enabled ? "Enabling" : "Disabling"} addon...`;

      try {
        await invoke("set_remote_addon_enabled", { manifestUrl, enabled });
        await reloadAddonDrivenViews();
        addonFeedbackEl.textContent = `Addon ${enabled ? "enabled" : "disabled"}.`;
      } catch (error) {
        addonFeedbackEl.textContent = String(error);
      }
    });
  });

  scope.querySelectorAll("[data-addon-remove]").forEach((button) => {
    button.addEventListener("click", async () => {
      const manifestUrl = button.dataset.addonRemove;
      addonFeedbackEl.textContent = "Removing addon...";

      try {
        await invoke("remove_remote_addon", { manifestUrl });
        await reloadAddonDrivenViews();
        addonFeedbackEl.textContent = "Addon removed.";
      } catch (error) {
        addonFeedbackEl.textContent = String(error);
      }
    });
  });

  scope.querySelectorAll("[data-addon-move]").forEach((button) => {
    button.addEventListener("click", async () => {
      const manifestUrl = button.dataset.addonSource;
      const direction = button.dataset.addonMove;
      addonFeedbackEl.textContent = `Moving addon ${direction}...`;

      try {
        await invoke("move_remote_addon", { manifestUrl, direction });
        await reloadAddonDrivenViews();
        addonFeedbackEl.textContent = `Addon moved ${direction}.`;
      } catch (error) {
        addonFeedbackEl.textContent = String(error);
      }
    });
  });
}

function renderAddonDetails() {
  const addon = installedAddons.find((item) => item.source === selectedAddonSource) ?? installedAddons[0];
  if (!addon) {
    addonDetailsEl.innerHTML = "";
    return;
  }

  addonDetailsEl.innerHTML = `
    <article class="addon-details-card">
      <div class="section-heading">
        <p class="eyebrow">Addon details</p>
        <h2>${escapeHtml(addon.name)}</h2>
      </div>
      <div class="addon-card-meta">
        <span class="provider-badge ${addon.enabled ? "is-success" : "is-neutral"}">${addon.enabled ? "Enabled" : "Disabled"}</span>
        <span class="provider-badge ${addon.configured ? "is-success" : "is-error"}">${addon.configured ? "Configured" : "Needs setup"}</span>
        <span class="provider-badge ${addonHealthClass(addon)}">${escapeHtml(addonHealthLabel(addon))}</span>
      </div>
      <p class="meta">${escapeHtml(addon.id)} • v${escapeHtml(addon.version || "unknown")} • ${escapeHtml(addon.transport)}</p>
      <p class="meta"><strong>Capabilities:</strong> ${escapeHtml(addon.capabilities.join(" / ") || "none")}</p>
      <p class="meta"><strong>Source:</strong> ${escapeHtml(addon.source)}</p>
      <p class="meta"><strong>Health:</strong> ${escapeHtml(addon.health_message)}</p>
      <p class="meta">${escapeHtml(addonSettingsHint(addon))}</p>
      ${
        addon.transport === "remote"
          ? `
            <div class="addon-actions addon-actions-detail">
              <button class="ghost-button addon-action" data-addon-open="${escapeHtml(addon.source)}">Open manifest</button>
              <button class="ghost-button addon-action" data-addon-move="up" data-addon-source="${escapeHtml(addon.source)}">Move up</button>
              <button class="ghost-button addon-action" data-addon-move="down" data-addon-source="${escapeHtml(addon.source)}">Move down</button>
              <button class="ghost-button addon-action" data-addon-toggle="${escapeHtml(addon.source)}" data-addon-enabled="${addon.enabled}">
                ${addon.enabled ? "Disable addon" : "Enable addon"}
              </button>
              <button class="ghost-button addon-action is-danger" data-addon-remove="${escapeHtml(addon.source)}">Remove addon</button>
            </div>
          `
          : `
            <div class="addon-actions addon-actions-detail">
              <button class="ghost-button addon-action" disabled>Built-in addon</button>
            </div>
          `
      }
    </article>
  `;
}

async function reloadAddonDrivenViews() {
  await renderAddons();
  await renderHome();
  await renderCatalog();

  const preferredId = selectedItemId || homeFeed?.hero?.id;
  if (!preferredId) {
    return;
  }

  try {
    await selectItem(preferredId);
  } catch (_error) {
    if (homeFeed?.hero?.id) {
      await selectItem(homeFeed.hero.id);
    }
  }
}

async function handleSearch(event) {
  const query = (event.target.value || "").trim();
  if (!query) {
    setSearchFeedback("");
    if (isSearchViewActive) {
      showMainView();
    }
    return;
  }

  setSearchFeedback(`Ready to search "${query}". Press Enter or Search.`);
}

async function runSearch(rawQuery = searchEl?.value ?? "") {
  const query = rawQuery.trim();
  if (!query) {
    setSearchFeedback("");
    showMainView();
    return;
  }

  setSearchFeedback(`Searching for "${query}"...`);
  let items = await invoke("search_catalog", { query });
  if (items.length === 0 && catalogItemsCache.length > 0) {
    items = filterCatalogItemsLocally(catalogItemsCache, query);
  }
  cacheItems(items);

  const filtered = activeFilter
    ? items.filter((item) => item.media_type === activeFilter)
    : items;

  if (filtered.length === 0) {
    lastExecutedSearch = query;
    setSearchFeedback(`No matches for "${query}".`);
    renderSearchEmpty(query);
    showSearchView();
    return;
  }

  lastExecutedSearch = query;
  setSearchFeedback(`${filtered.length} match${filtered.length === 1 ? "" : "es"} for "${query}".`);
  renderSearchResults(query, filtered);
  showSearchView();
}

function setSearchFeedback(message) {
  if (!searchFeedbackEl) {
    return;
  }
  searchFeedbackEl.textContent = message;
}

function filterCatalogItemsLocally(items, query) {
  const normalized = query.trim().toLowerCase();
  return items.filter((item) => {
    const genres = (item.genres || []).join(" ").toLowerCase();
    return (
      String(item.title || "").toLowerCase().includes(normalized)
      || String(item.description || "").toLowerCase().includes(normalized)
      || String(item.id || "").toLowerCase().includes(normalized)
      || genres.includes(normalized)
    );
  });
}

function renderCatalogEmpty(message) {
  catalogEl.innerHTML = `
    <article class="card catalog-empty">
      <p class="eyebrow">Catalog</p>
      <h3>No matches</h3>
      <p>${escapeHtml(message)}</p>
    </article>
  `;
}

function renderSearchResults(query, items) {
  searchResultsTitleEl.textContent = `Results for "${query}"`;
  searchResultsSummaryEl.textContent = `${items.length} match${items.length === 1 ? "" : "es"}.`;
  searchResultsEl.innerHTML = items.map(renderCard).join("");
  bindSearchResultButtons();
}

function renderSearchEmpty(query) {
  searchResultsTitleEl.textContent = `Results for "${query}"`;
  searchResultsSummaryEl.textContent = "No matches found.";
  searchResultsEl.innerHTML = `
    <article class="card catalog-empty">
      <p class="eyebrow">Search</p>
      <h3>No matches</h3>
      <p>No results for "${escapeHtml(query)}".</p>
    </article>
  `;
}

function bindSearchResultButtons() {
  searchResultsEl.querySelectorAll("[data-id]").forEach((button) => {
    button.addEventListener("click", async () => {
      const id = button.dataset.id;
      showPlayerView({ returnTo: "search" });
      await selectItem(id);
    });
  });
}

function showSearchView() {
  if (!mainViewEl || !searchViewEl || !playerViewEl || !settingsViewEl) {
    return;
  }
  closeSettingsModal();
  currentPage = "search";
  setPlayerFullscreen(false);
  isSearchViewActive = true;
  mainViewEl.classList.add("is-hidden");
  playerViewEl.classList.add("is-hidden");
  searchViewEl.classList.remove("is-hidden");
  window.scrollTo(0, 0);
}

function showMainView() {
  if (!mainViewEl || !searchViewEl || !playerViewEl || !settingsViewEl) {
    return;
  }
  closeSettingsModal();
  currentPage = "main";
  setPlayerFullscreen(false);
  isSearchViewActive = false;
  searchViewEl.classList.add("is-hidden");
  playerViewEl.classList.add("is-hidden");
  mainViewEl.classList.remove("is-hidden");
  window.scrollTo(0, 0);
}

function showPlayerView(options = {}) {
  if (!mainViewEl || !searchViewEl || !playerViewEl || !settingsViewEl) {
    return;
  }
  closeSettingsModal();
  const { returnTo = currentPage === "search" ? "search" : "main" } = options;
  playerReturnPage = returnTo;
  currentPage = "player";
  isSearchViewActive = false;
  mainViewEl.classList.add("is-hidden");
  searchViewEl.classList.add("is-hidden");
  playerViewEl.classList.remove("is-hidden");
  window.scrollTo(0, 0);
}

function toggleSettingsModal() {
  if (isSettingsModalOpen()) {
    closeSettingsModal();
    return;
  }
  openSettingsModal();
}

function openSettingsModal() {
  if (!settingsViewEl) {
    return;
  }
  settingsViewEl.classList.remove("is-hidden");
  settingsToggleEl?.setAttribute("aria-expanded", "true");
  document.body.classList.add("is-settings-open");
}

function closeSettingsModal() {
  if (!settingsViewEl) {
    return;
  }
  settingsViewEl.classList.add("is-hidden");
  settingsToggleEl?.setAttribute("aria-expanded", "false");
  document.body.classList.remove("is-settings-open");
}

function isSettingsModalOpen() {
  return Boolean(settingsViewEl && !settingsViewEl.classList.contains("is-hidden"));
}

function bindCatalogButtons(scope) {
  scope.querySelectorAll("[data-id]").forEach((button) => {
    button.addEventListener("click", async () => {
      const id = button.dataset.id;
      showPlayerView({ returnTo: currentPage === "search" ? "search" : "main" });
      await selectItem(id);
    });
  });
}

function bindHeroButtons() {
  heroEl.querySelectorAll("[data-play-hero], [data-open-hero]").forEach((button) => {
    button.addEventListener("click", async () => {
      const id = button.dataset.playHero ?? button.dataset.openHero;
      showPlayerView({ returnTo: "main" });
      await selectItem(id, { autoPlay: Boolean(button.dataset.playHero) });
    });
  });
}

async function selectItem(id, options = {}) {
  const { autoPlay = false } = options;
  const requestToken = ++selectItemRequestToken;
  renderPlayerLoadingState(id);
  try {
    const item = await getItem(id);
    if (requestToken !== selectItemRequestToken) {
      return;
    }
    if (id !== selectedItemId) {
      stopTorboxAutoRefresh();
      torboxSubmissionState = null;
      torboxDraftMagnet = "";
      torboxCachedOnly = true;
      manualSourceToolsVisible = false;
      autoPlayTrace = null;
    }
    resetPlaybackSession();
    selectedItemId = id;
    selectedLookup = await invoke("get_stream_lookup", { id });
    if (requestToken !== selectItemRequestToken) {
      return;
    }
    selectedStreams = selectedLookup.streams ?? [];
    selectedStreamIndex = 0;
    selectedStreamProviderFilter = "all";
    playbackActivated = Boolean(autoPlay);
    playbackPercent = 0;
    playbackCurrentSeconds = 0;
    playbackDurationSeconds = estimateRuntimeSeconds(item);
    lastPlaybackError = "";
    lastPlaybackNotice = "";
    if (selectedStreams.length > 0) {
      stopTorboxAutoRefresh();
      renderPlayer(item);
      renderStreams(item.title);
      if (autoPlay) {
        setPlaybackState(true);
      }
    } else {
      playbackActivated = false;
      setPlaybackState(false);
      renderNoStreams(item, selectedLookup);
      if (autoPlay) {
        await attemptAutoPlay(item);
      }
    }
  } catch (error) {
    if (requestToken !== selectItemRequestToken) {
      return;
    }
    renderShellError(String(error));
  }
}

function renderPlayerLoadingState(id) {
  const cached = itemCache.get(id);
  const title = cached?.title ? escapeHtml(cached.title) : "Loading title";
  const description = cached?.description
    ? escapeHtml(cached.description)
    : "Fetching metadata and stream sources...";

  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-art ${heroArtworkUrl(cached) ? "" : "is-fallback"}">
        ${cached ? renderArtworkImage(cached, "player-poster") : ""}
      </div>
      <div class="player-overlay player-overlay-loading">
        <p class="eyebrow">Player</p>
        <h2>${title}</h2>
        <p>${description}</p>
      </div>
    </div>
  `;

  playerDetailsEl.innerHTML = `
    <article class="player-details-card player-loading-card">
      <p class="eyebrow">Preparing playback</p>
      <p>Loading stream sources for this title...</p>
      <div class="loading-pulse"></div>
    </article>
  `;

  streamsEl.classList.add("empty");
  streamsEl.textContent = "Loading available stream sources...";
}

async function getItem(id) {
  try {
    const item = await invoke("get_media_item", { id });
    itemCache.set(id, item);
    return item;
  } catch (error) {
    if (itemCache.has(id)) {
      return itemCache.get(id);
    }
    throw error;
  }
}

function renderPlayer(item) {
  if (selectedStreams.length === 0) {
    renderNoStreams(item, selectedLookup);
    return;
  }

  const activeStream = selectedStreams[selectedStreamIndex];
  const quickSources = selectedStreams
    .map(
      (stream, index) => {
        const details = streamDetailLines(stream);
        const title = streamDisplayTitle(stream);
        const detailLine = details[0] || stream.playback_note || "";
        return `
          <button
            class="source-chooser-item ${index === selectedStreamIndex ? "is-active" : ""}"
            data-quick-stream-index="${index}"
            type="button"
          >
            <span class="source-chooser-title-row">
              <span class="source-chooser-quality">${escapeHtml(stream.quality || "Unknown quality")}</span>
              <span class="source-chooser-kind">${playbackKindLabel(stream)}</span>
            </span>
            <span class="source-chooser-name">${escapeHtml(title)}</span>
            <span class="source-chooser-meta">${escapeHtml(streamProviderName(stream))} • ${escapeHtml(stream.language || "Unknown language")}</span>
            ${detailLine ? `<span class="source-chooser-detail">${escapeHtml(detailLine)}</span>` : ""}
          </button>
        `;
      }
    )
    .join("");

  if (!playbackActivated) {
    playerStageEl.innerHTML = `
      <div class="player-screen">
        <div class="player-art ${heroArtworkUrl(item) ? "" : "is-fallback"}">
          ${renderArtworkImage(item, "player-poster")}
        </div>
        <div class="player-badges">
          <span class="badge">${item.media_type}</span>
          <span class="badge">${item.year}</span>
          <span class="badge">${escapeHtml(activeStream.quality || "Source ready")}</span>
        </div>

        <div class="player-overlay player-intro-card">
          <p class="eyebrow">Ready to watch</p>
          <h2>${item.title}</h2>
          <p>Pick a stream below to start playback. The player loads right after you choose one.</p>
          <p class="player-subtitle">${item.genres.join(" / ")}</p>
        </div>

        <aside class="source-chooser-overlay">
          <p class="eyebrow">Source options</p>
          <div class="source-chooser-list">
            ${quickSources}
          </div>
        </aside>
      </div>
    `;

    playerDetailsEl.innerHTML = `
      <article class="player-details-card">
        <p class="eyebrow">Selected title</p>
        <h3>${item.title}</h3>
        <p class="meta">${item.year} • ${item.media_type} • ${item.genres.join(" / ")}</p>
      </article>
      <article class="player-details-card">
        <p class="eyebrow">Current source</p>
        <h3>${escapeHtml(streamDisplayTitle(activeStream))}</h3>
        <p class="meta">${escapeHtml(streamProviderName(activeStream))} • ${escapeHtml(activeStream.quality || "Unknown quality")} • ${playbackKindLabel(activeStream)}</p>
      </article>
    `;

    playerStageEl.querySelectorAll("[data-quick-stream-index]").forEach((button) => {
      button.addEventListener("click", async () => {
        await switchToQuickSource(Number(button.dataset.quickStreamIndex), { autoPlay: true });
      });
    });
    return;
  }

  if (activeStream.playback_kind !== "embedded") {
    renderHandoffPlayer(item, activeStream);
    return;
  }
  const videoPoster = heroArtworkUrl(item) || item.poster_url;
  const escapedPoster = videoPoster ? `poster="${escapeHtml(videoPoster)}"` : "";
  const escapedVideoUrl = activeStream.url ? `data-playback-url="${escapeHtml(activeStream.url)}"` : "";

  playerStageEl.innerHTML = `
    <div class="player-screen is-video ${isPlaying && !isPlaybackStarting ? "is-playing" : ""}">
      <div class="player-video-shell ${heroArtworkUrl(item) ? "" : "is-fallback"}">
        <div class="player-art ${heroArtworkUrl(item) ? "" : "is-fallback"}">
          ${renderArtworkImage(item, "player-poster")}
        </div>
        <video
          id="player-video"
          class="player-video"
          preload="metadata"
          playsinline
          controlslist="nodownload noplaybackrate noremoteplayback"
          disablepictureinpicture
          ${escapedPoster}
          ${escapedVideoUrl}
        ></video>
      </div>
      <div class="player-badges">
        <span class="badge">${activeStream.quality}</span>
        <span class="badge" id="player-status-badge">${isPlaying ? "Playing now" : "Paused"}</span>
      </div>

      <div class="player-overlay player-overlay-compact">
        <h2>${item.title}</h2>
      </div>
      <div class="player-custom-controls" id="player-custom-controls">
        <button
          class="control-button player-control-chip"
          id="toggle-playback-mini"
          data-player-action="toggle"
          aria-label="${isPlaying ? "Pause" : "Play"}"
          title="${isPlaying ? "Pause" : "Play"}"
        >
          ${isPlaying ? "Pause" : "Play"}
        </button>
        <input
          id="player-seek"
          class="player-seek"
          type="range"
          min="0"
          max="1000"
          step="1"
          value="${Math.round(playbackPercent * 10)}"
          aria-label="Seek"
        />
        <span id="player-time" class="player-time">${formatDuration(playbackCurrentSeconds)} / ${formatDuration(playbackDurationSeconds || estimateRuntimeSeconds(item))}</span>
        <button
          class="control-button player-control-chip"
          id="toggle-sources-mini"
          data-player-action="sources"
          aria-label="Choose source"
          title="Choose source"
        >
          Sources
        </button>
        <button
          class="control-button player-fullscreen-mini"
          id="toggle-fullscreen-mini"
          data-player-action="fullscreen"
          aria-label="${isPlayerFullscreen ? "Exit fullscreen" : "Enter fullscreen"}"
          title="${isPlayerFullscreen ? "Exit fullscreen" : "Enter fullscreen"}"
        >
          <span aria-hidden="true">&#x26F6;</span>
        </button>
      </div>
      <aside id="player-source-overlay" class="source-chooser-overlay player-source-overlay is-hidden">
        <p class="eyebrow">Switch source</p>
        <div class="source-chooser-list">
          ${quickSources}
        </div>
      </aside>
    </div>
  `;

  playerDetailsEl.innerHTML = `
    <article class="player-details-card">
      <p class="eyebrow">Now selected</p>
      <h3>${item.title}</h3>
      <p class="meta">${item.year} • ${item.media_type} • ${item.genres.join(" / ")}</p>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Playback controls</p>
      <div class="control-row">
        <button class="control-button" data-player-action="rewind">-10s</button>
        <button class="control-button" data-player-action="toggle" id="toggle-playback">${isPlaying ? "Pause" : "Play"}</button>
        <button class="control-button" data-player-action="forward">+30s</button>
        <button class="control-button" data-player-action="fullscreen" id="toggle-fullscreen">${isPlayerFullscreen ? "Exit full screen" : "Full screen"}</button>
      </div>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Stream status</p>
      <p id="stream-status-message">${selectedLookup?.message ?? `Ready to play from ${activeStream.name}.`}</p>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Quick actions</p>
      <div class="control-buttons">
        <button class="ghost-button" data-player-action="restart">Restart</button>
        <button class="ghost-button" data-player-action="next-source">Next source</button>
      </div>
      <p class="meta">Next source cycles within: ${escapeHtml(nextSourceScopeLabel())}</p>
    </article>
  `;

  bindPlayerActions(item);
  bindQuickSourceOverlayButtons();
  bindPlayerSeekControls();
  mountPlayer(item, activeStream);
  syncPlayerUi(item, activeStream);
}

function bindQuickSourceOverlayButtons() {
  playerStageEl.querySelectorAll("[data-quick-stream-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const sourceOverlay = playerStageEl.querySelector("#player-source-overlay");
      sourceOverlay?.classList.add("is-hidden");
      await switchToQuickSource(Number(button.dataset.quickStreamIndex), {
        autoPlay: true,
      });
    });
  });
}

async function switchToQuickSource(nextIndex, options = {}) {
  const { autoPlay = true } = options;
  const resumeAt = getCurrentPlaybackSeconds();
  selectedStreamIndex = nextIndex;
  playbackActivated = true;
  pendingSeekSeconds = resumeAt;
  setPlaybackState(false);
  const selectedItem = await getItem(selectedItemId);
  renderPlayer(selectedItem);
  renderStreams(selectedItem.title);
  if (autoPlay) {
    setPlaybackState(true);
  }
}

function renderHandoffPlayer(item, stream) {
  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-art ${heroArtworkUrl(item) ? "" : "is-fallback"}">
        ${renderArtworkImage(item, "player-poster")}
      </div>
      <div class="player-badges">
        <span class="badge">${item.media_type}</span>
        <span class="badge">${item.year}</span>
        <span class="badge">${playbackKindLabel(stream)}</span>
        <span class="badge">${stream.quality}</span>
      </div>

      <div class="player-overlay">
        <p class="eyebrow">Player</p>
        <h2>${item.title}</h2>
        <p>${escapeHtml(stream.playback_note || "This source needs to open outside the embedded player.")}</p>
        <p class="player-subtitle">${item.genres.join(" / ")} • Source: ${stream.name} • Language: ${stream.language}</p>
      </div>
    </div>
  `;

  playerDetailsEl.innerHTML = `
    <article class="player-details-card">
      <p class="eyebrow">Now selected</p>
      <h3>${item.title}</h3>
      <p class="meta">${item.year} • ${item.media_type} • ${item.genres.join(" / ")}</p>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Source handling</p>
      <div class="provider-badge-row">
        <span class="provider-badge ${playbackKindClass(stream)}">${playbackKindLabel(stream)}</span>
      </div>
      <p id="stream-status-message">${lastPlaybackError || lastPlaybackNotice || stream.playback_note}</p>
      <div class="control-buttons">
        <button class="primary-button" id="handoff-open-source">${openSourceLabel(stream)}</button>
        <button class="ghost-button" data-player-action="next-source">Next source</button>
      </div>
      <p class="meta">Next source cycles within: ${escapeHtml(nextSourceScopeLabel())}</p>
    </article>
  `;

  playerDetailsEl.querySelector("#handoff-open-source")?.addEventListener("click", () => {
    void openStreamExternally(stream);
  });

  playerDetailsEl.querySelectorAll("[data-player-action]").forEach((button) => {
    button.addEventListener("click", () => {
      handlePlayerAction(button.dataset.playerAction, item);
    });
  });
}

function bindPlayerActions(item) {
  const actionButtons = [
    ...playerDetailsEl.querySelectorAll("[data-player-action]"),
    ...playerStageEl.querySelectorAll("[data-player-action]"),
  ];

  actionButtons.forEach((button) => {
    button.addEventListener("click", () => {
      handlePlayerAction(button.dataset.playerAction, item);
    });
  });
}

function bindPlayerSeekControls() {
  const seekField = playerStageEl.querySelector("#player-seek");
  if (!seekField) {
    return;
  }

  const applySeekFromRange = () => {
    const item = currentItem();
    if (!item) {
      return;
    }
    const duration = playbackDurationSeconds || estimateRuntimeSeconds(item);
    if (!duration || duration <= 0) {
      return;
    }
    const targetSeconds = (Number(seekField.value) / 1000) * duration;
    seekPlayerTo(targetSeconds);
  };

  seekField.addEventListener("input", applySeekFromRange);
  seekField.addEventListener("change", applySeekFromRange);
}

function handlePlayerAction(action, item) {
  if (action === "toggle") {
    setPlaybackState(!isPlaying);
  } else if (action === "rewind") {
    seekPlayer(-10);
  } else if (action === "forward") {
    seekPlayer(30);
  } else if (action === "restart") {
    seekPlayerTo(0);
  } else if (action === "fullscreen") {
    void toggleFullscreen();
  } else if (action === "sources") {
    const sourceOverlay = playerStageEl.querySelector("#player-source-overlay");
    sourceOverlay?.classList.toggle("is-hidden");
    return;
  } else if (action === "next-source" && selectedStreams.length > 0) {
    const resumeAt = getCurrentPlaybackSeconds();
    const shouldResume = isPlaying;
    selectedStreamIndex = nextStreamIndexForActiveFilter();
    playbackActivated = true;
    pendingSeekSeconds = resumeAt;
    lastPlaybackError = "";
    setPlaybackState(false);
    renderPlayer(item);
    renderStreams(item.title);

    if (shouldResume) {
      setPlaybackState(true);
    }
    return;
  }

  syncPlayerUi(item, activeStreamForSelection());
}

function nextStreamIndexForActiveFilter() {
  if (selectedStreams.length === 0) {
    return 0;
  }
  if (selectedStreamProviderFilter === "all") {
    return (selectedStreamIndex + 1) % selectedStreams.length;
  }

  const providerIndexes = selectedStreams
    .map((stream, index) => ({ stream, index }))
    .filter(({ stream }) => streamProviderName(stream) === selectedStreamProviderFilter)
    .map(({ index }) => index);

  if (providerIndexes.length === 0) {
    return (selectedStreamIndex + 1) % selectedStreams.length;
  }

  const currentPosition = providerIndexes.indexOf(selectedStreamIndex);
  if (currentPosition < 0) {
    return providerIndexes[0];
  }

  return providerIndexes[(currentPosition + 1) % providerIndexes.length];
}

function renderStreams(title) {
  if (selectedStreams.length === 0) {
    renderNoStreams(itemCache.get(selectedItemId), selectedLookup);
    return;
  }

  if (!playbackActivated) {
    streamsEl.classList.add("is-hidden");
    return;
  }

  const activeSource = selectedStreams[selectedStreamIndex];
  const lookupCandidates = (selectedLookup?.candidates ?? []).filter((candidate) => candidate?.magnet_url);
  const canSubmitCandidates = showAutomaticSourceActions();
  const providerOptions = ["all", ...new Set(selectedStreams.map(streamProviderName))];
  if (!providerOptions.includes(selectedStreamProviderFilter)) {
    selectedStreamProviderFilter = "all";
  }
  const visibleStreams = selectedStreams
    .map((stream, index) => ({ stream, index }))
    .filter(({ stream }) => selectedStreamProviderFilter === "all" || streamProviderName(stream) === selectedStreamProviderFilter);

  streamsEl.classList.remove("empty");
  streamsEl.classList.remove("is-hidden");
  streamsEl.innerHTML = `
    <p class="eyebrow">Stream sources</p>
    <h3>${title}</h3>
    <div class="stream-provider-tabs">
      ${providerOptions
        .map(
          (provider) => `
            <button
              class="stream-provider-tab ${provider === selectedStreamProviderFilter ? "is-active" : ""}"
              data-stream-provider="${escapeHtml(provider)}"
            >
              ${escapeHtml(streamProviderTabLabel(provider))}
            </button>
          `,
        )
        .join("")}
    </div>
    <div class="stream-list stream-option-list">
      ${visibleStreams
        .map(
          ({ stream, index }) => `
            <article class="stream-card stream-option-card ${index === selectedStreamIndex ? "is-active" : ""}">
              <div class="stream-option-top">
                <div>
                  <p class="stream-option-quality">${escapeHtml(stream.quality || "Unknown quality")}</p>
                  <h3>${escapeHtml(streamDisplayTitle(stream))}</h3>
                </div>
                <div class="provider-badge-row">
                  <span class="provider-badge ${playbackKindClass(stream)}">${playbackKindLabel(stream)}</span>
                </div>
              </div>
              <p class="stream-option-subtitle">${escapeHtml(streamProviderName(stream))} • ${escapeHtml(stream.language || "unknown language")} • ${playbackKindLabel(stream)}</p>
              <div class="stream-option-actions">
                <button class="stream-button ${index === selectedStreamIndex ? "is-active" : ""}" data-stream-index="${index}">
                  ${streamSelectionLabel(stream, index === selectedStreamIndex, playbackActivated)}
                </button>
                <button class="ghost-button stream-link" data-open-stream-index="${index}">${openSourceLabel(stream)}</button>
              </div>
              <details class="stream-advanced">
                <summary>More details</summary>
                <div class="stream-advanced-body">
                  <p class="stream-option-provider">${escapeHtml(streamProviderName(stream))}</p>
                  <p class="stream-option-note">${escapeHtml(stream.playback_note || "No extra source details yet.")}</p>
                  ${streamDetailLines(stream)
                    .map((detail) => `<p class="stream-option-detail">${escapeHtml(detail)}</p>`)
                    .join("")}
                </div>
              </details>
            </article>
          `,
        )
        .join("")}
    </div>
    ${
      lookupCandidates.length > 0
        ? `
          <div class="stream-candidate-block">
            <p class="eyebrow">More source options</p>
            <div class="stream-list stream-option-list">
              ${lookupCandidates
                .map(
                  (candidate, index) => `
                    <article class="stream-card stream-option-card">
                      <div class="stream-option-top">
                        <div>
                          <p class="stream-option-quality">Candidate</p>
                          <h3>${escapeHtml(candidate.name || "Addon source")}</h3>
                        </div>
                        <div class="provider-badge-row">
                          <span class="provider-badge is-pending">Magnet</span>
                        </div>
                      </div>
                      <p class="stream-option-subtitle">${escapeHtml(candidate.detail || "Addable source candidate")}</p>
                      <p class="stream-option-note">This option can be sent to TorBox to prepare playback.</p>
                      <div class="stream-option-actions">
                        <button
                          class="stream-button"
                          data-lookup-candidate-index="${index}"
                          ${canSubmitCandidates ? "" : "disabled"}
                        >
                          ${canSubmitCandidates ? "Send to TorBox" : "TorBox required"}
                        </button>
                      </div>
                    </article>
                  `,
                )
                .join("")}
            </div>
          </div>
        `
        : ""
    }
    <p class="stream-meta">
      Showing ${visibleStreams.length} option${visibleStreams.length === 1 ? "" : "s"}
      from ${selectedStreamProviderFilter === "all" ? "all providers" : streamProviderName(activeSource)}
    </p>
    <p class="stream-meta">Active source: ${escapeHtml(streamDisplayTitle(activeSource))} • ${escapeHtml(streamProviderName(activeSource))} • ${escapeHtml(activeSource.quality || "Unknown quality")} • ${playbackKindLabel(activeSource)}</p>
  `;

  streamsEl.querySelectorAll("[data-stream-provider]").forEach((button) => {
    button.addEventListener("click", () => {
      selectedStreamProviderFilter = button.dataset.streamProvider || "all";
      renderStreams(title);
    });
  });

  streamsEl.querySelectorAll("[data-stream-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const resumeAt = getCurrentPlaybackSeconds();
      const shouldResume = playbackActivated && isPlaying;
      selectedStreamIndex = Number(button.dataset.streamIndex);
      playbackActivated = true;
      const item = await getItem(selectedItemId);
      pendingSeekSeconds = resumeAt;
      setPlaybackState(false);
      renderPlayer(item);
      renderStreams(item.title);

      if (shouldResume) {
        setPlaybackState(true);
      }
    });
  });

  streamsEl.querySelectorAll("[data-open-stream-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const stream = selectedStreams[Number(button.dataset.openStreamIndex)];
      await openStreamExternally(stream);
    });
  });

  streamsEl.querySelectorAll("[data-lookup-candidate-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const candidateIndex = Number(button.dataset.lookupCandidateIndex);
      const candidate = lookupCandidates[candidateIndex];
      if (!candidate?.magnet_url || !selectedItemId) {
        return;
      }

      const item = await getItem(selectedItemId);
      torboxSubmissionState = {
        pending: true,
        status: "pending",
        message: `Sending "${candidate.name || "candidate"}" to TorBox...`,
      };
      renderStreams(item.title);

      try {
        const result = await invoke("submit_torbox_magnet", {
          id: item.id,
          magnet: candidate.magnet_url,
          onlyIfCached: torboxCachedOnly,
        });

        torboxSubmissionState = {
          pending: false,
          status: result.status,
          message: result.message,
        };

        await selectItem(item.id);
      } catch (error) {
        torboxSubmissionState = {
          pending: false,
          status: "error",
          message: String(error),
        };
        renderStreams(item.title);
      }
    });
  });
}

function streamProviderName(stream) {
  if (stream?.provider) {
    return stream.provider;
  }
  const [provider] = String(stream?.name ?? "").split(" • ");
  return provider?.trim() || "Unknown";
}

function streamDisplayTitle(stream) {
  if (stream?.full_title) {
    return stream.full_title;
  }
  const parts = String(stream?.name ?? "").split(" • ").map((part) => part.trim()).filter(Boolean);
  if (parts.length <= 1) {
    return stream?.name ?? "Unnamed stream";
  }
  return parts.slice(1).join(" • ");
}

function streamProviderTabLabel(provider) {
  return provider === "all" ? "All" : provider;
}

function nextSourceScopeLabel() {
  return selectedStreamProviderFilter === "all" ? "All providers" : selectedStreamProviderFilter;
}

function streamSourceLine(stream) {
  const parts = [];
  parts.push(streamProviderName(stream));
  parts.push(stream.language || "unknown");
  parts.push(playbackSummary(stream));

  const details = Array.isArray(stream?.details) ? stream.details.filter(Boolean) : [];
  if (details.length > 0) {
    parts.push(details[0]);
  }

  return parts.join(" • ");
}

function streamDetailLines(stream) {
  return Array.isArray(stream?.details) ? stream.details.filter(Boolean) : [];
}

function playbackSummary(stream) {
  if (stream?.playback_kind === "embedded") {
    return "ready to play";
  }
  if (stream?.playback_kind === "external") {
    return "opens externally";
  }
  if (stream?.playback_kind === "blocked") {
    return "blocked in app";
  }
  return "source";
}

function renderNoStreams(item, lookup) {
  const provider = lookup?.provider ?? "Streams";
  const message = lookup?.message ?? `No streams found for ${item.id}`;
  const candidates = lookup?.candidates ?? [];
  const acquisitionMessage = torboxSubmissionState?.message ?? "";
  const acquisitionStatus = String(torboxSubmissionState?.status ?? "").toLowerCase();
  const acquisitionPending = torboxSubmissionState?.pending ?? false;
  const showAutoPlayTools = showAutomaticSourceActions();
  const showTorboxActions = showAutoPlayTools && manualSourceToolsVisible;
  const traceSteps = autoPlayTrace?.itemId === item.id ? autoPlayTrace.steps : [];

  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-art ${heroArtworkUrl(item) ? "" : "is-fallback"}">
        ${renderArtworkImage(item, "player-poster")}
      </div>
      <div class="player-badges">
        <span class="badge">${item.media_type}</span>
        <span class="badge">${item.year}</span>
        <span class="badge">${provider}</span>
        <span class="badge">No stream</span>
      </div>

      <div class="player-overlay">
        <p class="eyebrow">Player</p>
        <h2>${item.title}</h2>
        <p>${message}</p>
        <p class="player-subtitle">Sol will try addon streams first, then automatic source acquisition if it can.</p>
      </div>
    </div>
  `;

  playerDetailsEl.innerHTML = `
    <article class="player-details-card">
      <p class="eyebrow">Now selected</p>
      <h3>${item.title}</h3>
      <p class="meta">${item.year} • ${item.media_type} • ${item.genres.join(" / ")}</p>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Playback controls</p>
      <div class="control-row">
        <button class="control-button" data-no-stream-action="rewind" disabled>-10s</button>
        <button class="control-button" data-no-stream-action="play">Play</button>
        <button class="control-button" data-no-stream-action="forward" disabled>+30s</button>
      </div>
      <p class="meta">Play will try addon/TorBox source candidates when no direct stream is available.</p>
    </article>

    <article class="player-details-card">
      <p class="eyebrow">Stream status</p>
      <p>${message}</p>
    </article>

    ${
      showAutoPlayTools
        ? `
          <article class="player-details-card">
            <p class="eyebrow">Autoplay</p>
            <p class="meta">Press Play and Sol will try direct streams first, then TorBox candidate handoff.</p>
            <div class="control-buttons">
              <button class="primary-button" id="autoplay-source" ${acquisitionPending || autoPlayPending ? "disabled" : ""}>
                Play
              </button>
              <button class="ghost-button" id="toggle-manual-source-tools">
                ${showTorboxActions ? "Hide manual tools" : "Show manual tools"}
              </button>
            </div>
            ${
              acquisitionMessage
                ? `<p class="submit-feedback ${escapeHtml(acquisitionStatus)}">${escapeHtml(acquisitionMessage)}</p>`
                : ""
            }
          </article>

          <article class="player-details-card">
            <p class="eyebrow">Autoplay trace</p>
            ${
              traceSteps.length > 0
                ? `
                  <div class="trace-list">
                    ${traceSteps
                      .map(
                        (step) => `
                          <div class="trace-step ${escapeHtml(step.kind)}">
                            <span class="trace-dot"></span>
                            <p>${escapeHtml(step.message)}</p>
                          </div>
                        `
                      )
                      .join("")}
                  </div>
                `
                : `<p class="meta">Press Play and Sol will log each autoplay step here.</p>`
            }
          </article>
        `
        : ""
    }

    ${
      showTorboxActions
        ? `
          <article class="player-details-card">
            <p class="eyebrow">Manual Source</p>
            <p class="meta">Use this only if autoplay could not find something usable.</p>
            <label class="torbox-form">
              <span class="sr-only">Magnet link</span>
              <textarea id="torbox-magnet" placeholder="magnet:?xt=urn:btih:...">${escapeHtml(torboxDraftMagnet)}</textarea>
            </label>
            <label class="checkbox-row">
              <input id="torbox-only-cached" type="checkbox" ${torboxCachedOnly ? "checked" : ""} />
              <span>Only add if already cached</span>
            </label>
            <div class="control-buttons">
              <button class="primary-button" id="torbox-submit-source" ${acquisitionPending ? "disabled" : ""}>
                ${acquisitionPending ? "Sending..." : "Send to TorBox"}
              </button>
              <button class="ghost-button" id="torbox-refresh-lookup">Refresh lookup</button>
            </div>
          </article>
        `
        : ""
    }
  `;

  streamsEl.classList.remove("empty");
  streamsEl.innerHTML = `
    <p class="eyebrow">TorBox lookup</p>
    <h3>${provider}</h3>
    <p class="stream-meta">${message}</p>
    ${
      candidates.length > 0
        ? `
          <div class="stream-list">
            ${candidates
              .map(
                (candidate) => `
                  <article class="stream-card">
                    <h3>${candidate.name}</h3>
                    <p class="stream-meta">${candidate.detail}</p>
                  </article>
                `,
              )
              .join("")}
          </div>
        `
        : `<p class="stream-meta">No close matches were found in your current TorBox library.</p>`
    }
  `;

  bindNoStreamActions(item, showTorboxActions);
}

function bindNoStreamActions(item, showTorboxActions) {
  const autoplayButton = document.querySelector("#autoplay-source");
  const toggleManualButton = document.querySelector("#toggle-manual-source-tools");
  const noStreamPlayButton = playerDetailsEl.querySelector("[data-no-stream-action='play']");

  noStreamPlayButton?.addEventListener("click", async () => {
    await attemptAutoPlay(item, { force: true });
  });

  autoplayButton?.addEventListener("click", async () => {
    await attemptAutoPlay(item, { force: true });
  });

  toggleManualButton?.addEventListener("click", () => {
    manualSourceToolsVisible = !manualSourceToolsVisible;
    renderNoStreams(item, selectedLookup);
  });

  if (!showTorboxActions) {
    return;
  }

  const submitButton = document.querySelector("#torbox-submit-source");
  const refreshButton = document.querySelector("#torbox-refresh-lookup");
  const magnetField = document.querySelector("#torbox-magnet");
  const cachedOnlyField = document.querySelector("#torbox-only-cached");

  submitButton?.addEventListener("click", async () => {
    const magnet = magnetField?.value?.trim() ?? "";
    const onlyIfCached = cachedOnlyField?.checked ?? true;
    torboxDraftMagnet = magnet;
    torboxCachedOnly = onlyIfCached;

    torboxSubmissionState = {
      pending: true,
      status: "pending",
      message: "Sending magnet to TorBox...",
    };
    renderNoStreams(item, selectedLookup);

    try {
      const result = await invoke("submit_torbox_magnet", {
        id: item.id,
        magnet,
        onlyIfCached,
      });

      torboxSubmissionState = {
        pending: false,
        status: result.status,
        message: result.message,
      };

      await selectItem(item.id);
      if (selectedStreams.length === 0) {
        startTorboxAutoRefresh(item.id, { autoPlay: true });
        renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);
      }
    } catch (error) {
      stopTorboxAutoRefresh();
      torboxSubmissionState = {
        pending: false,
        status: "error",
        message: String(error),
      };
      renderNoStreams(item, selectedLookup);
    }
  });

  refreshButton?.addEventListener("click", async () => {
    torboxDraftMagnet = magnetField?.value ?? torboxDraftMagnet;
    torboxCachedOnly = cachedOnlyField?.checked ?? torboxCachedOnly;
    torboxSubmissionState = {
      pending: false,
      status: "refresh",
      message: "Refreshing TorBox lookup...",
    };
    stopTorboxAutoRefresh();
    await selectItem(item.id);
    if (selectedStreams.length === 0) {
      startTorboxAutoRefresh(item.id);
      renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);
    }
  });
}

function resetAutoPlayTrace(itemId) {
  autoPlayTrace = {
    itemId,
    steps: [],
  };
}

function pushAutoPlayTrace(itemId, message, kind = "neutral") {
  if (selectedItemId !== itemId) {
    return;
  }

  if (!autoPlayTrace || autoPlayTrace.itemId !== itemId) {
    resetAutoPlayTrace(itemId);
  }

  autoPlayTrace.steps.push({ message, kind });
  if (autoPlayTrace.steps.length > 8) {
    autoPlayTrace.steps = autoPlayTrace.steps.slice(-8);
  }
}

function startTorboxAutoRefresh(itemId, options = {}) {
  const { autoPlay = false } = options;
  stopTorboxAutoRefresh();
  torboxAutoRefreshAttempt = 0;

  const poll = async () => {
    if (selectedItemId !== itemId) {
      stopTorboxAutoRefresh();
      return;
    }

    torboxAutoRefreshAttempt += 1;
    torboxSubmissionState = {
      pending: false,
      status: "refresh",
      message: `Waiting for TorBox to prepare this item. Auto-refreshing (${torboxAutoRefreshAttempt}/${TORBOX_AUTO_REFRESH_MAX_ATTEMPTS})...`,
    };
    pushAutoPlayTrace(itemId, `Waiting for TorBox (${torboxAutoRefreshAttempt}/${TORBOX_AUTO_REFRESH_MAX_ATTEMPTS}).`, "pending");

    await selectItem(itemId);

    if (selectedStreams.length > 0) {
      torboxSubmissionState = {
        pending: false,
        status: "ready",
        message: "TorBox has a playable stream ready.",
      };
      pushAutoPlayTrace(itemId, "TorBox returned a playable stream.", "success");
      if (autoPlay) {
        playbackActivated = true;
        const refreshedItem = itemCache.get(itemId);
        if (refreshedItem) {
          renderPlayer(refreshedItem);
        }
        setPlaybackState(true);
      }
      return;
    }

    if (torboxAutoRefreshAttempt >= TORBOX_AUTO_REFRESH_MAX_ATTEMPTS) {
      stopTorboxAutoRefresh();
      torboxSubmissionState = {
        pending: false,
        status: "timeout",
        message: "Still waiting on TorBox. Try Refresh lookup again in a little while.",
      };
      pushAutoPlayTrace(itemId, "Timed out waiting for TorBox to prepare a stream.", "error");
      const item = itemCache.get(itemId);
      if (item) {
        renderNoStreams(item, selectedLookup);
      }
      return;
    }

    torboxAutoRefreshTimer = window.setTimeout(() => {
      void poll();
    }, TORBOX_AUTO_REFRESH_INTERVAL_MS);
  };

  torboxAutoRefreshTimer = window.setTimeout(() => {
    void poll();
  }, TORBOX_AUTO_REFRESH_INTERVAL_MS);
}

function stopTorboxAutoRefresh() {
  if (torboxAutoRefreshTimer) {
    window.clearTimeout(torboxAutoRefreshTimer);
    torboxAutoRefreshTimer = null;
  }
  torboxAutoRefreshAttempt = 0;
}

function cacheItems(items) {
  items.forEach((item) => itemCache.set(item.id, item));
}

function showAutomaticSourceActions() {
  const torboxAddon = installedAddons.find((addon) => addon.id === "builtin.torbox");
  return Boolean(torboxAddon?.enabled && torboxAddon?.configured);
}

async function attemptAutoPlay(item, options = {}) {
  const { force = false } = options;
  if (autoPlayPending || selectedItemId !== item.id) {
    return;
  }

  if (!force && selectedStreams.length > 0) {
    resetAutoPlayTrace(item.id);
    pushAutoPlayTrace(item.id, "Using an existing playable stream.", "success");
    playbackActivated = true;
    renderPlayer(item);
    setPlaybackState(true);
    return;
  }

  if (!showAutomaticSourceActions()) {
    resetAutoPlayTrace(item.id);
    pushAutoPlayTrace(item.id, "Autoplay cannot continue because TorBox is not configured.", "error");
    torboxSubmissionState = {
      pending: false,
      status: "auto_unavailable",
      message: "Automatic playback needs TorBox configured, or another addon with a directly playable stream.",
    };
    renderNoStreams(item, selectedLookup);
    return;
  }

  autoPlayPending = true;
  resetAutoPlayTrace(item.id);
  pushAutoPlayTrace(item.id, "Play pressed. Checking addon-provided streams and source candidates.", "pending");
  torboxSubmissionState = {
    pending: true,
    status: "searching",
    message: "Trying to find a playable source automatically...",
  };
  renderNoStreams(item, selectedLookup);

  try {
    const lookupCandidates = (selectedLookup?.candidates ?? []).filter((candidate) => candidate.magnet_url);
    let release = lookupCandidates[0] ?? null;

    if (!release) {
      pushAutoPlayTrace(item.id, "No addable source candidates were returned for this title.", "error");
    } else {
      pushAutoPlayTrace(item.id, `Found an addon-provided source candidate: ${release.name || release.title}.`, "success");
    }

    if (!release?.magnet_url) {
      torboxSubmissionState = {
        pending: false,
        status: "auto_unavailable",
        message: selectedLookup?.message || "Sol could not find an automatic source for this title yet.",
      };
      manualSourceToolsVisible = true;
      renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);
      return;
    }

    torboxDraftMagnet = release.magnet_url;
    pushAutoPlayTrace(item.id, `Submitting ${release.title || release.name} to TorBox.`, "pending");
    torboxSubmissionState = {
      pending: true,
      status: "pending",
      message: `Trying ${release.title}...`,
    };
    renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);

    const result = await invoke("submit_torbox_magnet", {
      id: item.id,
      magnet: release.magnet_url,
      onlyIfCached: torboxCachedOnly,
    });

    torboxSubmissionState = {
      pending: false,
      status: result.status,
      message: result.message,
    };
    pushAutoPlayTrace(
      item.id,
      result.message,
      result.status === "ready" || result.status === "submitted" || result.status === "submitted_cached_only"
        ? "success"
        : "pending"
    );

    await selectItem(item.id);
    if (selectedStreams.length > 0) {
      pushAutoPlayTrace(item.id, "A playable stream is ready and playback is starting.", "success");
      playbackActivated = true;
      renderPlayer(itemCache.get(item.id) ?? item);
      setPlaybackState(true);
      return;
    }

    pushAutoPlayTrace(item.id, "Waiting for TorBox to prepare a playable stream.", "pending");
    startTorboxAutoRefresh(item.id, { autoPlay: true });
    renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);
  } catch (error) {
    stopTorboxAutoRefresh();
    pushAutoPlayTrace(item.id, `Autoplay failed: ${String(error)}`, "error");
    torboxSubmissionState = {
      pending: false,
      status: "error",
      message: String(error),
    };
    manualSourceToolsVisible = true;
    renderNoStreams(itemCache.get(item.id) ?? item, selectedLookup);
  } finally {
    autoPlayPending = false;
  }
}

function setPlaybackState(nextState) {
  const video = document.querySelector("#player-video");
  const item = currentItem();
  const stream = activeStreamForSelection();

  if (!item || !stream) {
    isPlaying = false;
    isPlaybackStarting = false;
    syncPlayerUi(item, stream);
    return;
  }

  if (nextState) {
    const playbackBlockReason = getPlaybackBlockReason(stream);
    if (playbackBlockReason) {
      if (stream?.playback_kind === "external" || stream?.playback_kind === "blocked") {
        void openStreamExternally(stream, { fromPlayAction: true });
        return;
      }
      isPlaying = false;
      isPlaybackStarting = false;
      lastPlaybackError = playbackBlockReason;
      lastPlaybackNotice = "";
      lastPlaybackNoticeKind = "";
      syncPlayerUi(item, stream);
      return;
    }
  }

  if (video) {
    if (nextState) {
      isPlaybackStarting = true;
      lastPlaybackError = "";
      lastPlaybackNotice = "";
      lastPlaybackNoticeKind = "";
      armPlaybackStartWatchdog(item, stream, video);
      video.play().catch((error) => {
        isPlaying = false;
        isPlaybackStarting = false;
        clearPlaybackStartWatchdog();
        lastPlaybackError = `Playback could not start: ${error.message ?? error}`;
        lastPlaybackNotice = "";
        lastPlaybackNoticeKind = "";
        syncPlayerUi(item, stream);
      });
    } else {
      isPlaying = false;
      isPlaybackStarting = false;
      clearPlaybackStartWatchdog();
      video.pause();
    }
  } else if (nextState) {
    isPlaying = false;
    isPlaybackStarting = false;
  }

  syncPlayerUi(item, stream);
}

function mountPlayer(item, stream) {
  const video = document.querySelector("#player-video");
  if (!video) {
    return;
  }

  const playbackUrl = video.dataset.playbackUrl || stream.url || "";
  if (video.src !== playbackUrl) {
    video.src = playbackUrl;
    video.load();
  }

  video.addEventListener("click", () => {
    setPlaybackState(!isPlaying);
  });

  const initialSeekSeconds = pendingSeekSeconds;
  video.addEventListener("loadedmetadata", () => {
    playbackDurationSeconds = Number.isFinite(video.duration) && video.duration > 0
      ? video.duration
      : estimateRuntimeSeconds(item);

    if (initialSeekSeconds !== null) {
      video.currentTime = Math.min(initialSeekSeconds, playbackDurationSeconds || initialSeekSeconds);
      pendingSeekSeconds = null;
    }

    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("canplay", () => {
    if (!lastPlaybackError) {
      lastPlaybackNotice = "Stream loaded. Starting playback...";
      lastPlaybackNoticeKind = "info";
      syncPlayerUi(item, stream);
    }
  });

  video.addEventListener("loadeddata", () => {
    lastPlaybackNotice = "First video frame loaded.";
    lastPlaybackNoticeKind = "info";
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("playing", () => {
    clearPlaybackStartWatchdog();
    isPlaybackStarting = false;
    isPlaying = true;
    lastPlaybackError = "";
    lastPlaybackNotice = "Video is rendering in the embedded player.";
    lastPlaybackNoticeKind = "info";
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("timeupdate", () => {
    if (video.currentTime > 0) {
      clearPlaybackStartWatchdog();
      isPlaybackStarting = false;
      isPlaying = true;
    }
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("play", () => {
    if (!isPlaybackStarting) {
      isPlaybackStarting = true;
    }
    isPlaying = false;
    lastPlaybackError = "";
    lastPlaybackNotice = "Starting playback...";
    lastPlaybackNoticeKind = "info";
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("pause", () => {
    if (!video.ended) {
      clearPlaybackStartWatchdog();
      isPlaybackStarting = false;
      isPlaying = false;
      syncPlaybackFromVideo(item, stream, video);
    }
  });

  video.addEventListener("ended", () => {
    clearPlaybackStartWatchdog();
    isPlaybackStarting = false;
    isPlaying = false;
    playbackCurrentSeconds = playbackDurationSeconds || video.duration || playbackCurrentSeconds;
    playbackPercent = 100;
    syncPlayerUi(item, stream);
  });

  video.addEventListener("error", () => {
    clearPlaybackStartWatchdog();
    isPlaybackStarting = false;
    isPlaying = false;
    lastPlaybackError = describeVideoError(video, stream);
    lastPlaybackNotice = "";
    lastPlaybackNoticeKind = "";
    syncPlayerUi(item, stream);
  });

  video.addEventListener("waiting", () => {
    if (video.currentTime <= 0 && !isPlaybackStarting) {
      isPlaybackStarting = true;
      armPlaybackStartWatchdog(item, stream, video);
    }
    if (!lastPlaybackError) {
      lastPlaybackNotice = "Waiting for the stream to buffer...";
      lastPlaybackNoticeKind = "info";
      syncPlayerUi(item, stream);
    }
  });

  video.addEventListener("stalled", () => {
    if (video.currentTime <= 0 && !isPlaybackStarting) {
      isPlaybackStarting = true;
      armPlaybackStartWatchdog(item, stream, video);
    }
    if (!lastPlaybackError) {
      lastPlaybackNotice = "The stream stalled before the player could render it.";
      lastPlaybackNoticeKind = "info";
      syncPlayerUi(item, stream);
    }
  });

  syncPlayerUi(item, stream);
}

function syncPlaybackFromVideo(item, stream, video) {
  playbackCurrentSeconds = Number.isFinite(video.currentTime) ? video.currentTime : 0;
  playbackDurationSeconds = Number.isFinite(video.duration) && video.duration > 0
    ? video.duration
    : estimateRuntimeSeconds(item);
  playbackPercent = playbackDurationSeconds > 0
    ? Math.min(100, (playbackCurrentSeconds / playbackDurationSeconds) * 100)
    : 0;
  syncPlayerUi(item, stream);
}

function syncPlayerUi(item, stream) {
  if (!item || !stream) {
    return;
  }

  const videoScreen = playerStageEl.querySelector(".player-screen.is-video");
  const statusBadge = document.querySelector("#player-status-badge");
  const subtitle = document.querySelector("#player-subtitle");
  const toggleButtons = document.querySelectorAll("[data-player-action='toggle']");
  const fullscreenButtons = [
    ...playerDetailsEl.querySelectorAll("[data-player-action='fullscreen']"),
    ...playerStageEl.querySelectorAll("[data-player-action='fullscreen']"),
  ];
  const streamStatusMessage = document.querySelector("#stream-status-message");
  const seekField = playerStageEl.querySelector("#player-seek");
  const timeLabel = playerStageEl.querySelector("#player-time");

  if (videoScreen) {
    videoScreen.classList.toggle("is-playing", isPlaying && !isPlaybackStarting);
  }

  if (statusBadge) {
    statusBadge.textContent = playbackStatusLabel();
  }

  if (subtitle) {
    subtitle.textContent = `${item.genres.join(" / ")} • Source: ${stream.name} • Language: ${stream.language}`;
  }

  toggleButtons.forEach((button) => {
    const label = isPlaybackStarting ? "Starting..." : isPlaying ? "Pause" : "Play";
    button.textContent = label;
    button.setAttribute("aria-label", label);
    button.setAttribute("title", label);
  });

  fullscreenButtons.forEach((button) => {
    const isMiniButton = button.id === "toggle-fullscreen-mini";
    const label = isPlayerFullscreen ? "Exit fullscreen" : "Enter fullscreen";
    button.setAttribute("aria-label", label);
    button.setAttribute("title", label);
    if (!isMiniButton) {
      button.textContent = isPlayerFullscreen ? "Exit full screen" : "Full screen";
    }
  });

  if (streamStatusMessage) {
    streamStatusMessage.textContent =
      lastPlaybackError ||
      lastPlaybackNotice ||
      (isPlaybackStarting ? `Trying to start playback from ${stream.name}...` : "") ||
      selectedLookup?.message ||
      `Ready to play from ${stream.name}.`;
  }

  if (seekField) {
    seekField.value = String(Math.round(playbackPercent * 10));
  }

  if (timeLabel) {
    const elapsedSeconds = Number.isFinite(playbackCurrentSeconds) ? playbackCurrentSeconds : 0;
    const totalSeconds = Number.isFinite(playbackDurationSeconds) && playbackDurationSeconds > 0
      ? playbackDurationSeconds
      : estimateRuntimeSeconds(item);
    timeLabel.textContent = `${formatDuration(elapsedSeconds)} / ${formatDuration(totalSeconds)}`;
  }
}

function playbackStatusLabel() {
  if (lastPlaybackError) {
    return "Playback issue";
  }

  if (lastPlaybackNoticeKind === "external") {
    return "External handoff";
  }

  if (isPlaybackStarting) {
    return "Starting";
  }

  if (lastPlaybackNoticeKind === "info" && /waiting|buffer/i.test(lastPlaybackNotice)) {
    return "Buffering";
  }

  if (isPlaying) {
    return "Playing now";
  }

  if (playbackPercent >= 100) {
    return "Ended";
  }

  return "Paused";
}

function seekPlayer(deltaSeconds) {
  const video = document.querySelector("#player-video");
  const nextTime = getCurrentPlaybackSeconds() + deltaSeconds;
  seekPlayerTo(nextTime, video);
}

function seekPlayerTo(targetSeconds, video = document.querySelector("#player-video")) {
  const item = currentItem();
  const stream = activeStreamForSelection();
  if (!item || !stream) {
    return;
  }

  const duration = playbackDurationSeconds || estimateRuntimeSeconds(item);
  const boundedTime = Math.max(0, Math.min(targetSeconds, duration || targetSeconds));

  playbackCurrentSeconds = boundedTime;
  playbackPercent = duration > 0 ? Math.min(100, (boundedTime / duration) * 100) : 0;

  if (video) {
    video.currentTime = boundedTime;
  }

  syncPlayerUi(item, stream);
}

function getCurrentPlaybackSeconds() {
  const video = document.querySelector("#player-video");
  if (video && Number.isFinite(video.currentTime)) {
    return video.currentTime;
  }

  return playbackCurrentSeconds;
}

function currentItem() {
  return selectedItemId ? itemCache.get(selectedItemId) ?? null : null;
}

function activeStreamForSelection() {
  return selectedStreams[selectedStreamIndex] ?? null;
}

function resetPlaybackSession() {
  const video = document.querySelector("#player-video");
  if (video) {
    video.pause();
    video.removeAttribute("src");
    video.load();
  }

  clearPlaybackStartWatchdog();
  isPlaying = false;
  isPlaybackStarting = false;
  playbackCurrentSeconds = 0;
  playbackDurationSeconds = 0;
  playbackPercent = 0;
  pendingSeekSeconds = null;
  lastPlaybackError = "";
  lastPlaybackNotice = "";
  lastPlaybackNoticeKind = "";
}

function formatPlaybackTime(item) {
  const totalSeconds = playbackDurationSeconds || estimateRuntimeSeconds(item);
  const elapsedSeconds = Math.min(playbackCurrentSeconds, totalSeconds || playbackCurrentSeconds);
  return `${playbackStatusLabel()} • ${formatDuration(elapsedSeconds)} / ${formatDuration(totalSeconds)}`;
}

function armPlaybackStartWatchdog(item, stream, video) {
  clearPlaybackStartWatchdog();
  playbackStartTimer = window.setTimeout(() => {
    if (!isPlaybackStarting || isPlaying) {
      return;
    }

    isPlaybackStarting = false;
    isPlaying = false;
    if (!video.paused) {
      video.pause();
    }

    const playbackBlockReason = getPlaybackBlockReason(stream);
    lastPlaybackError = playbackBlockReason
      || "This source did not become playable in the embedded player. Try Next source or Open source URL.";
    lastPlaybackNotice = "";
    lastPlaybackNoticeKind = "";
    syncPlayerUi(item, stream);
  }, PLAYBACK_START_TIMEOUT_MS);
}

function clearPlaybackStartWatchdog() {
  if (playbackStartTimer) {
    window.clearTimeout(playbackStartTimer);
    playbackStartTimer = null;
  }
}

function getPlaybackBlockReason(stream) {
  if (!stream?.url) {
    return "This source does not include a media URL for embedded playback.";
  }

  if (stream.playback_kind === "external") {
    return stream.playback_note || "This source opens outside the embedded player.";
  }

  if (stream.playback_kind === "blocked") {
    return stream.playback_note || "This source cannot be embedded in the app.";
  }

  if (stream.url.startsWith("http://")) {
    return "This source uses plain HTTP, and the embedded player blocks insecure media here. Use Open source URL or switch to another source.";
  }

  return "";
}

async function toggleFullscreen() {
  if (currentPage !== "player" || !playerStageEl?.querySelector("#player-video")) {
    return;
  }

  if (invoke) {
    try {
      const nextState = await invoke("toggle_window_fullscreen");
      isNativeFullscreen = Boolean(nextState);
      setPlayerFullscreen(isNativeFullscreen);
      return;
    } catch (_error) {
      // Fall back to in-app fullscreen mode if native fullscreen is unavailable.
    }
  }

  setPlayerFullscreen(!isPlayerFullscreen);
}

function tauriCurrentWindow() {
  return window.__TAURI__?.window?.getCurrentWindow?.() ?? null;
}

async function toggleWindowMaximize() {
  if (invoke) {
    try {
      await invoke("toggle_window_maximize");
      return;
    } catch (_error) {
      // Fall through to API fallback.
    }
  }

  const currentWindow = tauriCurrentWindow();
  if (currentWindow?.isMaximized && currentWindow?.maximize && currentWindow?.unmaximize) {
    try {
      const maximized = await currentWindow.isMaximized();
      if (maximized) {
        await currentWindow.unmaximize();
      } else {
        await currentWindow.maximize();
      }
      return;
    } catch (_error) {
      // Fall through to element fullscreen when window API is unavailable in this runtime.
    }
  }

  await toggleFullscreen();
}

function setPlayerFullscreen(nextState) {
  isPlayerFullscreen = Boolean(nextState);
  document.body.classList.toggle("is-player-fullscreen", isPlayerFullscreen);
  document.body.classList.toggle("is-player-controls-visible", isPlayerFullscreen);

  if (fullscreenControlsTimer) {
    window.clearTimeout(fullscreenControlsTimer);
    fullscreenControlsTimer = null;
  }

  if (isPlayerFullscreen) {
    scheduleFullscreenControlsHide();
  }

  const item = currentItem();
  const stream = activeStreamForSelection();
  if (item && stream) {
    syncPlayerUi(item, stream);
  }
}

function showFullscreenControls() {
  const now = performance.now();
  if (now - lastFullscreenControlsRefreshMs < 180) {
    return;
  }
  lastFullscreenControlsRefreshMs = now;
  document.body.classList.add("is-player-controls-visible");
  scheduleFullscreenControlsHide();
}

function scheduleFullscreenControlsHide() {
  if (!isPlayerFullscreen) {
    return;
  }

  if (fullscreenControlsTimer) {
    window.clearTimeout(fullscreenControlsTimer);
  }

  fullscreenControlsTimer = window.setTimeout(() => {
    if (!isPlayerFullscreen) {
      return;
    }
    document.body.classList.remove("is-player-controls-visible");
  }, 1800);
}

async function openStreamExternally(stream, options = {}) {
  const { fromPlayAction = false, autoFallback = false } = options;
  const item = currentItem();
  if (!stream?.url || !invoke) {
    return;
  }

  try {
    await invoke("open_external_url", { url: stream.url });
    isPlaying = false;
    isPlaybackStarting = false;
    clearPlaybackStartWatchdog();
    lastPlaybackError = "";
    lastPlaybackNotice = autoFallback
      ? `Embedded playback stalled, so ${stream.name} was opened outside the app.`
      : fromPlayAction
        ? `${stream.name} was opened outside the app.`
        : `Opened ${stream.name} outside the app.`;
    lastPlaybackNoticeKind = "external";
    syncPlayerUi(item, stream);
  } catch (error) {
    isPlaying = false;
    isPlaybackStarting = false;
    clearPlaybackStartWatchdog();
    lastPlaybackError = `Could not open this source externally: ${error.message ?? error}`;
    lastPlaybackNotice = "";
    lastPlaybackNoticeKind = "";
    syncPlayerUi(item, stream);
  }
}

function describeVideoError(video, stream) {
  const playbackBlockReason = getPlaybackBlockReason(stream);
  if (playbackBlockReason) {
    return playbackBlockReason;
  }

  const mediaError = video.error;
  if (!mediaError) {
    return "The selected stream could not be loaded in the embedded player.";
  }

  switch (mediaError.code) {
    case MediaError.MEDIA_ERR_ABORTED:
      return "Playback was interrupted before the stream could start.";
    case MediaError.MEDIA_ERR_NETWORK:
      return "The stream could not be loaded because of a network or server issue.";
    case MediaError.MEDIA_ERR_DECODE:
      return "The stream loaded, but this embedded player could not decode it.";
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      return "This source format is not supported by the embedded player.";
    default:
      return "The selected stream could not be loaded in the embedded player.";
  }
}

function estimateRuntimeSeconds(item) {
  if (item.media_type === "channel") {
    return 180 * 60;
  }

  if (item.media_type === "series") {
    return 52 * 60;
  }

  return 124 * 60;
}

function formatDuration(totalSeconds) {
  const roundedSeconds = Math.max(0, Math.round(totalSeconds || 0));
  const hours = Math.floor(roundedSeconds / 3600);
  const minutes = Math.floor((roundedSeconds % 3600) / 60);
  const seconds = roundedSeconds % 60;

  if (hours === 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}`;
  }

  return `${hours}:${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

function renderCard(item) {
  const mediaTypeLabel = formatMediaType(item.media_type);
  const showMediaBadge = mediaTypeLabel !== "Movie";
  const genres = Array.isArray(item.genres) ? item.genres.slice(0, 2) : [];
  const genreLine = genres.join(" / ");
  return `
    <article class="card">
      <button data-id="${item.id}">
        <div class="poster ${item.poster_url ? "" : "is-fallback"}">
          ${renderPosterImage(item, "poster-image")}
          ${showMediaBadge ? `<span class="poster-label">${escapeHtml(mediaTypeLabel)}</span>` : ""}
        </div>
        <h3>${item.title}</h3>
        <p class="meta">${item.year}${genreLine ? ` • ${escapeHtml(genreLine)}` : ""}</p>
        <p class="card-description">${escapeHtml(item.description || "")}</p>
      </button>
    </article>
  `;
}

function renderPosterImage(item, className) {
  if (!item.poster_url) {
    return "";
  }

  return `<img class="${className}" src="${item.poster_url}" alt="${escapeHtml(item.title)} poster" loading="lazy" />`;
}

function renderArtworkImage(item, className) {
  const artworkUrl = heroArtworkUrl(item);
  if (!artworkUrl) {
    return "";
  }

  return `<img class="${className}" src="${artworkUrl}" alt="${escapeHtml(item.title)} artwork" loading="lazy" />`;
}

function heroArtworkUrl(item) {
  return item?.backdrop_url || item?.poster_url || "";
}

function formatMediaType(mediaType) {
  const normalized = String(mediaType ?? "").trim().toLowerCase();
  if (!normalized) {
    return "Media";
  }
  return normalized.charAt(0).toUpperCase() + normalized.slice(1);
}

function playbackKindLabel(stream) {
  switch (stream?.playback_kind) {
    case "embedded":
      return "Embedded";
    case "external":
      return "External";
    case "blocked":
      return "Blocked in app";
    default:
      return "Unknown";
  }
}

function playbackKindClass(stream) {
  switch (stream?.playback_kind) {
    case "embedded":
      return "is-success";
    case "external":
      return "is-pending";
    case "blocked":
      return "is-error";
    default:
      return "is-neutral";
  }
}

function openSourceLabel(stream) {
  return stream?.playback_kind === "embedded" ? "Open source URL" : "Open externally";
}

function streamSelectionLabel(stream, isSelected, hasStartedPlayback = false) {
  if (stream?.playback_kind === "external") {
    return isSelected ? "Selected external source" : "Use external source";
  }

  if (stream?.playback_kind === "blocked") {
    return isSelected ? "Selected blocked source" : "Use blocked source";
  }

  if (!hasStartedPlayback) {
    return isSelected ? "Play selected source" : "Play this source";
  }

  return isSelected ? "Selected source" : "Switch source";
}

function addonSettingsHint(addon) {
  if (addon.transport === "remote") {
    return "Remote addons can be reordered, disabled, or removed here.";
  }

  if (addon.source.startsWith("env:")) {
    return `This built-in addon is configured through ${addon.source.replace("env:", "")}.`;
  }

  return "This built-in addon ships with Sol.";
}

function addonHealthLabel(addon) {
  switch (addon.health_status) {
    case "healthy":
      return "Healthy";
    case "setup_required":
      return "Needs setup";
    case "disabled":
      return "Disabled";
    case "error":
      return "Check addon";
    default:
      return "Unknown health";
  }
}

function addonHealthClass(addon) {
  switch (addon.health_status) {
    case "healthy":
      return "is-success";
    case "setup_required":
      return "is-pending";
    case "disabled":
      return "is-neutral";
    case "error":
      return "is-error";
    default:
      return "is-neutral";
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function renderShellError(message) {
  stopTorboxAutoRefresh();
  streamsEl.classList.add("empty");
  streamsEl.textContent = message;
  playerDetailsEl.innerHTML = "";
  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-overlay">
        <p class="eyebrow">Player</p>
        <h2>Waiting for a title</h2>
        <p>${message}</p>
      </div>
    </div>
  `;
}

bootstrap().catch((error) => renderShellError(String(error)));
