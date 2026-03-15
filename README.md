# sol

`sol` is a desktop-first Rust starter for a Stremio-like media platform.

The project is now moving toward an addon-first architecture so the app shell can stay lightweight while metadata, streams, and source discovery come from optional providers.

The first pass gives you:

- a minimal `tauri` desktop shell with a static frontend
- seeded catalog, search, home-feed, and stream data
- provider and service layers so metadata and streams can come from different backends
- an optional `axum` server binary for API experimentation

## Run the desktop app

```bash
cargo run
```

That launches the Tauri shell and shows:

- a featured hero card
- a trending rail
- a filterable catalog
- stream source inspection for each seeded title

## Run the API server

```bash
cargo run --bin server
```

Then open `http://127.0.0.1:3000` and try:

- `GET /api/home`
- `GET /api/catalog`
- `GET /api/catalog?type=movie`
- `GET /api/meta/movie:solstice`
- `GET /api/search?q=sci-fi`
- `GET /api/streams/movie:solstice`

## Addon shape

The app now follows a Stremio-like addon split:

- addons expose one or more capabilities like `catalog`, `meta`, `stream`, `source_search`, or `submit`
- the app aggregates those capabilities through an addon registry
- local env-based integrations are treated as builtin addons for now
- future remote HTTP addons can slot into the same registry without changing the desktop shell

Today the builtin addon registry includes:

- `Demo Catalog` for seeded catalog and fallback streams
- `TMDB Metadata` when `TMDB_API_READ_TOKEN` or `TMDB_API_KEY` is set
- `TorBox Streams` when `TORBOX_API_KEY` is set
- `Prowlarr Search` when `PROWLARR_URL` and `PROWLARR_API_KEY` are set

## Environment variables

To switch the movie catalog over to TMDB:

```bash
export TMDB_API_READ_TOKEN=your_tmdb_read_token
```

or:

```bash
export TMDB_API_KEY=your_tmdb_api_key
```

Optional for the next stream step:

```bash
export TORBOX_API_KEY=your_torbox_api_key
```

Optional for automatic source discovery:

```bash
export PROWLARR_URL=http://127.0.0.1:9696
export PROWLARR_API_KEY=your_prowlarr_api_key
```

## Current addon behavior

For TMDB-backed movie items, the builtin addons now:

1. Reads your TorBox torrent library.
2. Tries to match the selected movie by title and year.
3. Picks the largest video file in the best match.
4. Requests a TorBox stream URL for that file.

If TorBox has no matching item yet, the app shows a clearer “no stream yet” state with the closest matches it found in your current TorBox library.

When a title still has no stream, the desktop app now also lets you:

1. Paste a magnet link into the no-stream panel.
2. Send it directly to TorBox.
3. Keep the action in cached-only mode by default, or turn that off if you explicitly want TorBox to queue the torrent.
4. Refresh the lookup and try playback again once TorBox has the item ready.

If Prowlarr is configured, the app also searches release candidates automatically for no-stream titles and lets you send one straight to TorBox from the UI.

You can inspect the current addon registry through:

- `GET /api/addons`

## Suggested next steps

1. Add real metadata providers and a local library/watch history store with SQLite.
2. Introduce a shared playback/session layer that both desktop and future Android TV clients can consume.
3. Replace the static frontend with a richer Tauri UI or a bundled frontend framework once the machine has a working Node toolchain.
4. Add authentication, watchlists, progress tracking, and provider integrations.
# sol
