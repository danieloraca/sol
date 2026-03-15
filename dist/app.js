const invoke = window.__TAURI__?.core?.invoke;

const heroEl = document.querySelector("#hero");
const playerStageEl = document.querySelector("#player-stage");
const playerDetailsEl = document.querySelector("#player-details");
const trendingEl = document.querySelector("#trending");
const continueWatchingEl = document.querySelector("#continue-watching");
const catalogEl = document.querySelector("#catalog");
const streamsEl = document.querySelector("#streams");
const searchEl = document.querySelector("#search");
const filterButtons = [...document.querySelectorAll(".filter")];
const TORBOX_AUTO_REFRESH_INTERVAL_MS = 5000;
const TORBOX_AUTO_REFRESH_MAX_ATTEMPTS = 24;

let activeFilter = "";
let itemCache = new Map();
let homeFeed = null;
let selectedItemId = null;
let selectedStreams = [];
let selectedLookup = null;
let selectedStreamIndex = 0;
let playbackPercent = 0;
let isPlaying = false;
let playbackCurrentSeconds = 0;
let playbackDurationSeconds = 0;
let pendingSeekSeconds = null;
let lastPlaybackError = "";
let torboxSubmissionState = null;
let torboxDraftMagnet = "";
let torboxCachedOnly = true;
let torboxAutoRefreshTimer = null;
let torboxAutoRefreshAttempt = 0;
let sourceSearchState = null;

async function bootstrap() {
  if (!invoke) {
    renderShellError("Tauri runtime not detected. Launch this through `cargo run` to use the desktop shell.");
    return;
  }

  await renderHome();
  await renderCatalog();
  await selectItem(homeFeed.hero.id);

  searchEl.addEventListener("input", handleSearch);
  filterButtons.forEach((button) => {
    button.addEventListener("click", async () => {
      activeFilter = button.dataset.filter ?? "";
      filterButtons.forEach((item) => item.classList.toggle("is-active", item === button));
      searchEl.value = "";
      await renderCatalog();
    });
  });
}

async function renderHome() {
  homeFeed = await invoke("get_home_feed");
  cacheItems([homeFeed.hero, ...homeFeed.trending, ...homeFeed.continue_watching]);

  heroEl.innerHTML = `
    <div class="hero-media ${homeFeed.hero.poster_url ? "" : "is-fallback"}">
      ${renderPosterImage(homeFeed.hero, "hero-poster")}
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

  cacheItems(items);
  catalogEl.innerHTML = items.map(renderCard).join("");
  bindCatalogButtons(catalogEl);
}

async function handleSearch(event) {
  const query = event.target.value.trim();
  if (!query) {
    await renderCatalog();
    return;
  }

  const items = await invoke("search_catalog", { query });
  cacheItems(items);

  const filtered = activeFilter
    ? items.filter((item) => item.media_type === activeFilter)
    : items;

  catalogEl.innerHTML = filtered.map(renderCard).join("");
  bindCatalogButtons(catalogEl);
}

function bindCatalogButtons(scope) {
  scope.querySelectorAll("[data-id]").forEach((button) => {
    button.addEventListener("click", async () => {
      const id = button.dataset.id;
      await selectItem(id);
    });
  });
}

function bindHeroButtons() {
  heroEl.querySelectorAll("[data-play-hero], [data-open-hero]").forEach((button) => {
    button.addEventListener("click", async () => {
      const id = button.dataset.playHero ?? button.dataset.openHero;
      await selectItem(id);
      if (button.dataset.playHero) {
        setPlaybackState(true);
      }
    });
  });
}

async function selectItem(id) {
  try {
    const item = await getItem(id);
    if (id !== selectedItemId) {
      stopTorboxAutoRefresh();
      torboxSubmissionState = null;
      torboxDraftMagnet = "";
      torboxCachedOnly = true;
      sourceSearchState = null;
    }
    resetPlaybackSession();
    selectedItemId = id;
    selectedLookup = await invoke("get_stream_lookup", { id });
    selectedStreams = selectedLookup.streams ?? [];
    selectedStreamIndex = 0;
    playbackPercent = 0;
    playbackCurrentSeconds = 0;
    playbackDurationSeconds = estimateRuntimeSeconds(item);
    lastPlaybackError = "";
    if (selectedStreams.length > 0) {
      stopTorboxAutoRefresh();
      renderPlayer(item);
      renderStreams(item.title);
    } else {
      setPlaybackState(false);
      renderNoStreams(item, selectedLookup);
      if (selectedLookup?.provider === "TorBox") {
        void ensureSourceSearch(id);
      }
    }
  } catch (error) {
    renderShellError(String(error));
  }
}

async function getItem(id) {
  if (itemCache.has(id)) {
    return itemCache.get(id);
  }

  const item = await invoke("get_media_item", { id });
  itemCache.set(id, item);
  return item;
}

function renderPlayer(item) {
  if (selectedStreams.length === 0) {
    renderNoStreams(item, selectedLookup);
    return;
  }

  const activeStream = selectedStreams[selectedStreamIndex];
  const escapedPoster = item.poster_url ? `poster="${escapeHtml(item.poster_url)}"` : "";

  playerStageEl.innerHTML = `
    <div class="player-screen is-video">
      <div class="player-video-shell ${item.poster_url ? "" : "is-fallback"}">
        <div class="player-art ${item.poster_url ? "" : "is-fallback"}">
          ${renderPosterImage(item, "player-poster")}
        </div>
        <video id="player-video" class="player-video" preload="metadata" playsinline ${escapedPoster}>
          <source src="${escapeHtml(activeStream.url)}" />
        </video>
      </div>
      <div class="player-badges">
        <span class="badge">${item.media_type}</span>
        <span class="badge">${item.year}</span>
        <span class="badge">${activeStream.quality}</span>
        <span class="badge" id="player-status-badge">${isPlaying ? "Playing now" : "Paused"}</span>
      </div>

      <div class="player-overlay">
        <p class="eyebrow">Player</p>
        <h2>${item.title}</h2>
        <p>${item.description}</p>
        <p class="player-subtitle" id="player-subtitle">${item.genres.join(" / ")} • Source: ${activeStream.name} • Language: ${activeStream.language}</p>
      </div>

      <div class="player-progress">
        <div class="progress-meta" id="progress-meta">${formatPlaybackTime(item)}</div>
        <div class="progress-bar">
          <div class="progress-value" id="progress-value" style="width: ${Math.round(playbackPercent)}%"></div>
        </div>
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
        <button class="control-button" data-player-action="rewind">-10s</button>
        <button class="control-button" data-player-action="toggle" id="toggle-playback">${isPlaying ? "Pause" : "Play"}</button>
        <button class="control-button" data-player-action="forward">+30s</button>
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
    </article>
  `;

  bindPlayerActions(item);
  mountPlayer(item, activeStream);
  syncPlayerUi(item, activeStream);
}

function bindPlayerActions(item) {
  playerDetailsEl.querySelectorAll("[data-player-action]").forEach((button) => {
    button.addEventListener("click", () => {
      handlePlayerAction(button.dataset.playerAction, item);
    });
  });
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
  } else if (action === "next-source" && selectedStreams.length > 0) {
    const resumeAt = getCurrentPlaybackSeconds();
    const shouldResume = isPlaying;
    selectedStreamIndex = (selectedStreamIndex + 1) % selectedStreams.length;
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

function renderStreams(title) {
  if (selectedStreams.length === 0) {
    renderNoStreams(itemCache.get(selectedItemId), selectedLookup);
    return;
  }

  const activeSource = selectedStreams[selectedStreamIndex];
  streamsEl.classList.remove("empty");
  streamsEl.innerHTML = `
    <p class="eyebrow">Stream sources</p>
    <h3>${title}</h3>
    <div class="stream-list">
      ${selectedStreams
        .map(
          (stream, index) => `
            <article class="stream-card ${index === selectedStreamIndex ? "is-active" : ""}">
              <h3>${stream.name}</h3>
              <p class="stream-meta">${stream.quality} • ${stream.language}</p>
              <button class="stream-button ${index === selectedStreamIndex ? "is-active" : ""}" data-stream-index="${index}">
                ${index === selectedStreamIndex ? "Selected source" : "Switch to source"}
              </button>
              <a class="stream-link" href="${stream.url}" target="_blank" rel="noreferrer">Open source URL</a>
            </article>
          `,
        )
        .join("")}
    </div>
    <p class="stream-meta">Active source: ${activeSource.name} at ${activeSource.quality}</p>
  `;

  streamsEl.querySelectorAll("[data-stream-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const resumeAt = getCurrentPlaybackSeconds();
      const shouldResume = isPlaying;
      selectedStreamIndex = Number(button.dataset.streamIndex);
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
}

function renderNoStreams(item, lookup) {
  const provider = lookup?.provider ?? "Streams";
  const message = lookup?.message ?? `No streams found for ${item.id}`;
  const candidates = lookup?.candidates ?? [];
  const acquisitionMessage = torboxSubmissionState?.message ?? "";
  const acquisitionStatus = String(torboxSubmissionState?.status ?? "").toLowerCase();
  const acquisitionPending = torboxSubmissionState?.pending ?? false;
  const showTorboxActions = provider === "TorBox";
  const sourceSearch = sourceSearchState?.itemId === item.id
    ? sourceSearchState
    : {
        itemId: item.id,
        provider: "Prowlarr",
        status: "idle",
        message: "Search releases to find something you can send to TorBox.",
        releases: [],
        pending: false,
      };

  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-art ${item.poster_url ? "" : "is-fallback"}">
        ${renderPosterImage(item, "player-poster")}
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
        <p class="player-subtitle">Metadata is loaded, but playback needs a matching TorBox item in your library.</p>
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
      <p class="eyebrow">Stream status</p>
      <p>${message}</p>
    </article>

    ${
      showTorboxActions
        ? `
          <article class="player-details-card">
            <p class="eyebrow">Add Source</p>
            <p class="meta">Paste a magnet link and send it to TorBox. Cached-only is enabled by default so this won’t silently start a full download.</p>
            <div class="provider-badge-row">
              <span class="provider-badge ${sourceSearchBadgeClass(sourceSearch)}">${escapeHtml(sourceSearchBadgeLabel(sourceSearch))}</span>
            </div>
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
            ${
              acquisitionMessage
                ? `<p class="submit-feedback ${escapeHtml(acquisitionStatus)}">${escapeHtml(acquisitionMessage)}</p>`
                : ""
            }
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
    ${
      showTorboxActions
        ? `
          <div class="source-search-block">
            <p class="eyebrow">Source search</p>
            <h3>${escapeHtml(sourceSearch.provider ?? "Prowlarr")}</h3>
            <p class="stream-meta">${escapeHtml(sourceSearch.message ?? "Search for releases to add.")}</p>
            <div class="control-buttons">
              <button class="ghost-button" id="source-search-refresh" ${sourceSearch.pending ? "disabled" : ""}>
                ${sourceSearch.pending ? "Searching..." : "Search again"}
              </button>
            </div>
            ${
              (sourceSearch.releases ?? []).length > 0
                ? `
                  <div class="stream-list">
                    ${sourceSearch.releases
                      .map(
                        (release, index) => `
                          <article class="stream-card">
                            <h3>${escapeHtml(release.title)}</h3>
                            <p class="stream-meta">${escapeHtml(release.indexer)} • ${escapeHtml(release.protocol)} • ${escapeHtml(release.quality)}</p>
                            <p class="stream-meta">${escapeHtml(release.size)} • ${escapeHtml(release.seeders)} seeders • ${escapeHtml(release.age)}</p>
                            <button class="stream-button" data-source-index="${index}">
                              Send to TorBox
                            </button>
                          </article>
                        `,
                      )
                      .join("")}
                  </div>
                `
                : ""
            }
          </div>
        `
        : ""
    }
  `;

  bindNoStreamActions(item, showTorboxActions);
}

function bindNoStreamActions(item, showTorboxActions) {
  if (!showTorboxActions) {
    return;
  }

  const submitButton = document.querySelector("#torbox-submit-source");
  const refreshButton = document.querySelector("#torbox-refresh-lookup");
  const searchAgainButton = document.querySelector("#source-search-refresh");
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

  searchAgainButton?.addEventListener("click", async () => {
    await ensureSourceSearch(item.id, { force: true });
  });

  streamsEl.querySelectorAll("[data-source-index]").forEach((button) => {
    button.addEventListener("click", async () => {
      const index = Number(button.dataset.sourceIndex);
      const release = sourceSearchState?.releases?.[index];
      if (!release?.magnet_url) {
        return;
      }

      torboxDraftMagnet = release.magnet_url;
      torboxSubmissionState = {
        pending: true,
        status: "pending",
        message: `Sending "${release.title}" to TorBox...`,
      };
      renderNoStreams(item, selectedLookup);

      try {
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
  });
}

async function ensureSourceSearch(itemId, options = {}) {
  const { force = false } = options;
  if (selectedItemId !== itemId) {
    return;
  }

  if (
    !force &&
    sourceSearchState?.itemId === itemId &&
    (sourceSearchState.pending || sourceSearchState.status === "ready" || sourceSearchState.status === "no_results")
  ) {
    return;
  }

  sourceSearchState = {
    itemId,
    provider: "Prowlarr",
    status: "searching",
    message: "Searching Prowlarr for release candidates...",
    releases: [],
    pending: true,
  };

  const item = itemCache.get(itemId);
  if (item && selectedStreams.length === 0) {
    renderNoStreams(item, selectedLookup);
  }

  try {
    const result = await invoke("search_sources", { id: itemId });
    if (selectedItemId !== itemId) {
      return;
    }

    sourceSearchState = {
      itemId,
      ...result,
      pending: false,
    };
  } catch (error) {
    if (selectedItemId !== itemId) {
      return;
    }

    sourceSearchState = {
      itemId,
      provider: "Prowlarr",
      status: "error",
      message: String(error),
      releases: [],
      pending: false,
    };
  }

  const currentItem = itemCache.get(itemId);
  if (currentItem && selectedItemId === itemId && selectedStreams.length === 0) {
    renderNoStreams(currentItem, selectedLookup);
  }
}

function sourceSearchBadgeLabel(sourceSearch) {
  switch (sourceSearch.status) {
    case "searching":
      return "Prowlarr searching";
    case "ready":
      return `Prowlarr connected • ${sourceSearch.releases.length} results`;
    case "no_results":
      return "Prowlarr connected • no results";
    case "unavailable":
      return "Prowlarr not configured";
    case "request_failed":
    case "error":
      return "Prowlarr connection issue";
    default:
      return "Prowlarr idle";
  }
}

function sourceSearchBadgeClass(sourceSearch) {
  switch (sourceSearch.status) {
    case "ready":
    case "no_results":
      return "is-success";
    case "searching":
      return "is-pending";
    case "unavailable":
    case "request_failed":
    case "error":
      return "is-error";
    default:
      return "is-neutral";
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

    await selectItem(itemId);

    if (selectedStreams.length > 0) {
      torboxSubmissionState = {
        pending: false,
        status: "ready",
        message: "TorBox has a playable stream ready.",
      };
      if (autoPlay) {
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

function setPlaybackState(nextState) {
  isPlaying = nextState;
  const video = document.querySelector("#player-video");

  if (video) {
    if (isPlaying) {
      video.play().catch((error) => {
        isPlaying = false;
        lastPlaybackError = `Playback could not start: ${error.message ?? error}`;
        syncPlayerUi(currentItem(), activeStreamForSelection());
      });
    } else {
      video.pause();
    }
  } else if (isPlaying) {
    isPlaying = false;
  }

  syncPlayerUi(currentItem(), activeStreamForSelection());
}

function mountPlayer(item, stream) {
  const video = document.querySelector("#player-video");
  if (!video) {
    return;
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

  video.addEventListener("timeupdate", () => {
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("play", () => {
    isPlaying = true;
    lastPlaybackError = "";
    syncPlaybackFromVideo(item, stream, video);
  });

  video.addEventListener("pause", () => {
    if (!video.ended) {
      isPlaying = false;
      syncPlaybackFromVideo(item, stream, video);
    }
  });

  video.addEventListener("ended", () => {
    isPlaying = false;
    playbackCurrentSeconds = playbackDurationSeconds || video.duration || playbackCurrentSeconds;
    playbackPercent = 100;
    syncPlayerUi(item, stream);
  });

  video.addEventListener("error", () => {
    isPlaying = false;
    lastPlaybackError = "The selected stream could not be loaded in the embedded player.";
    syncPlayerUi(item, stream);
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

  const statusBadge = document.querySelector("#player-status-badge");
  const subtitle = document.querySelector("#player-subtitle");
  const progressMeta = document.querySelector("#progress-meta");
  const progressValue = document.querySelector("#progress-value");
  const toggleButton = document.querySelector("#toggle-playback");
  const streamStatusMessage = document.querySelector("#stream-status-message");

  if (statusBadge) {
    statusBadge.textContent = playbackStatusLabel();
  }

  if (subtitle) {
    subtitle.textContent = `${item.genres.join(" / ")} • Source: ${stream.name} • Language: ${stream.language}`;
  }

  if (progressMeta) {
    progressMeta.textContent = formatPlaybackTime(item);
  }

  if (progressValue) {
    progressValue.style.width = `${Math.round(playbackPercent)}%`;
  }

  if (toggleButton) {
    toggleButton.textContent = isPlaying ? "Pause" : "Play";
  }

  if (streamStatusMessage) {
    streamStatusMessage.textContent = lastPlaybackError || selectedLookup?.message || `Ready to play from ${stream.name}.`;
  }
}

function playbackStatusLabel() {
  if (lastPlaybackError) {
    return "Playback issue";
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

  isPlaying = false;
  playbackCurrentSeconds = 0;
  playbackDurationSeconds = 0;
  playbackPercent = 0;
  pendingSeekSeconds = null;
  lastPlaybackError = "";
}

function formatPlaybackTime(item) {
  const totalSeconds = playbackDurationSeconds || estimateRuntimeSeconds(item);
  const elapsedSeconds = Math.min(playbackCurrentSeconds, totalSeconds || playbackCurrentSeconds);
  return `${playbackStatusLabel()} • ${formatDuration(elapsedSeconds)} / ${formatDuration(totalSeconds)}`;
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
  return `
    <article class="card">
      <button data-id="${item.id}">
        <div class="poster ${item.poster_url ? "" : "is-fallback"}">
          ${renderPosterImage(item, "poster-image")}
          <span class="poster-label">${item.media_type}</span>
        </div>
        <h3>${item.title}</h3>
        <p class="meta">${item.year} • ${item.genres.join(" / ")}</p>
        <p>${item.description}</p>
        <p class="meta">${item.id === selectedItemId ? "Open in player" : "Select for playback"}</p>
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
