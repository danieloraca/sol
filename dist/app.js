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

let activeFilter = "";
let itemCache = new Map();
let homeFeed = null;
let selectedItemId = null;
let selectedStreams = [];
let selectedLookup = null;
let selectedStreamIndex = 0;
let playbackPercent = 18;
let isPlaying = false;
let playbackTimer = null;

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
    selectedItemId = id;
    selectedLookup = await invoke("get_stream_lookup", { id });
    selectedStreams = selectedLookup.streams ?? [];
    selectedStreamIndex = 0;
    playbackPercent = defaultProgressFor(item);
    setPlaybackState(false);
    if (selectedStreams.length > 0) {
      renderPlayer(item);
      renderStreams(item.title);
    } else {
      renderNoStreams(item, selectedLookup);
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
  const progressWidth = `${Math.round(playbackPercent)}%`;

  playerStageEl.innerHTML = `
    <div class="player-screen">
      <div class="player-art ${item.poster_url ? "" : "is-fallback"}">
        ${renderPosterImage(item, "player-poster")}
      </div>
      <div class="player-badges">
        <span class="badge">${item.media_type}</span>
        <span class="badge">${item.year}</span>
        <span class="badge">${activeStream.quality}</span>
        <span class="badge">${isPlaying ? "Playing now" : "Paused"}</span>
      </div>

      <div class="player-overlay">
        <p class="eyebrow">Player</p>
        <h2>${item.title}</h2>
        <p>${item.description}</p>
        <p class="player-subtitle">${item.genres.join(" / ")} • Source: ${activeStream.name} • Language: ${activeStream.language}</p>
      </div>

      <div class="player-progress">
        <div class="progress-meta">${formatPlaybackTime(playbackPercent, item)}</div>
        <div class="progress-bar">
          <div class="progress-value" style="width: ${progressWidth}"></div>
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
        <button class="control-button" data-player-action="toggle">${isPlaying ? "Pause" : "Play"}</button>
        <button class="control-button" data-player-action="forward">+30s</button>
      </div>
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
    playbackPercent = Math.max(0, playbackPercent - 3);
  } else if (action === "forward") {
    playbackPercent = Math.min(100, playbackPercent + 7);
  } else if (action === "restart") {
    playbackPercent = 0;
  } else if (action === "next-source" && selectedStreams.length > 0) {
    selectedStreamIndex = (selectedStreamIndex + 1) % selectedStreams.length;
  }

  renderPlayer(item);
  renderStreams(item.title);
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
      selectedStreamIndex = Number(button.dataset.streamIndex);
      const item = await getItem(selectedItemId);
      renderPlayer(item);
      renderStreams(item.title);
    });
  });
}

function renderNoStreams(item, lookup) {
  const provider = lookup?.provider ?? "Streams";
  const message = lookup?.message ?? `No streams found for ${item.id}`;
  const candidates = lookup?.candidates ?? [];

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
}

function cacheItems(items) {
  items.forEach((item) => itemCache.set(item.id, item));
}

function defaultProgressFor(item) {
  if (item.media_type === "channel") {
    return 64;
  }

  if (item.media_type === "series") {
    return 42;
  }

  return 18;
}

function setPlaybackState(nextState) {
  isPlaying = nextState;

  if (playbackTimer) {
    window.clearInterval(playbackTimer);
    playbackTimer = null;
  }

  if (!isPlaying) {
    return;
  }

  playbackTimer = window.setInterval(async () => {
    if (!selectedItemId) {
      return;
    }

    playbackPercent = Math.min(100, playbackPercent + 0.6);
    const item = await getItem(selectedItemId);
    renderPlayer(item);

    if (playbackPercent >= 100) {
      setPlaybackState(false);
      renderPlayer(item);
    }
  }, 1000);
}

function formatPlaybackTime(percent, item) {
  const totalMinutes = estimateRuntimeMinutes(item);
  const elapsedMinutes = Math.round((percent / 100) * totalMinutes);
  return `${isPlaying ? "Playing" : "Paused"} • ${formatMinutes(elapsedMinutes)} / ${formatMinutes(totalMinutes)}`;
}

function estimateRuntimeMinutes(item) {
  if (item.media_type === "channel") {
    return 180;
  }

  if (item.media_type === "series") {
    return 52;
  }

  return 124;
}

function formatMinutes(totalMinutes) {
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours === 0) {
    return `${minutes}m`;
  }

  return `${hours}h ${String(minutes).padStart(2, "0")}m`;
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
