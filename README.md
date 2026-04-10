# sol

`sol` is a desktop-first Rust starter for a Stremio-like media platform.

The project is now moving toward an addon-first architecture so the app shell can stay lightweight while metadata, streams, and source discovery come from optional providers.

The first pass gives you:

- a minimal `tauri` desktop shell with a static frontend
- seeded catalog, search, home-feed, and stream data
- provider and service layers so metadata and streams can come from different backends
- SQLite-backed watch progress for Continue Watching state
- an optional `axum` server binary for API experimentation

## Run the desktop app

```bash
cargo run
```

That launches the Tauri shell and shows:

- a top bar with media filters, search, and settings
- a featured hero section
- continue watching and trending rails
- a poster-first catalog grid
- search results view with dedicated back navigation
- a dedicated playback screen with custom controls and source switching
- a settings modal for addon install/manage actions

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
- `GET /api/watch-progress`
- `PUT /api/watch-progress/movie:solstice`
- `DELETE /api/watch-progress/movie:solstice`

Example upsert payload:

```json
{
  "progress_percent": 48.7,
  "position_seconds": 3640,
  "duration_seconds": 7480
}
```

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

Optional watch progress database override:

```bash
export SOL_DB_PATH=/absolute/path/to/sol.sqlite3
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

You can inspect the current addon registry through:

- `GET /api/addons`

## Watch progress storage

- Desktop playback progress is persisted in SQLite and powers the Continue Watching block.
- By default, Sol stores the database in your OS local data directory (`.../sol/sol.sqlite3`).
- Set `SOL_DB_PATH` if you want to use a shared path across devices/instances.
- The desktop app and the HTTP API now use the same watch-progress store.

## Suggested next steps

1. Add real metadata providers and user identity so watch progress can be isolated per user.
2. Introduce a shared playback/session layer that both desktop and future Android TV clients can consume.
3. Continue refining the desktop UX and interaction model, then carry that system into an Android TV-focused client.
4. Add authentication, watchlists, and deeper provider integrations.
