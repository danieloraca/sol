# sol

`sol` is a desktop-first Rust starter for a Stremio-like media platform.

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

## Provider shape

The app now follows a Stremio-like split:

- `MetadataProvider`: home feed, catalog rows, search, item details
- `StreamProvider`: playback sources for a selected media ID

Today the app uses:

- `SeededLibraryProvider` for metadata
- `TmdbMetadataProvider` when `TMDB_API_READ_TOKEN` or `TMDB_API_KEY` is set
- `TorboxStreamProvider` for real stream lookup against your TorBox torrent library when `TORBOX_API_KEY` is set
- `FallbackStreamProvider` so seeded demo streams still work when TorBox has no usable match

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

## Current TorBox behavior

For TMDB-backed movie items, the app now:

1. Reads your TorBox torrent library.
2. Tries to match the selected movie by title and year.
3. Picks the largest video file in the best match.
4. Requests a TorBox stream URL for that file.

If TorBox has no matching item yet, the app falls back to the seeded demo stream data for demo-only titles.

## Suggested next steps

1. Add real metadata providers and a local library/watch history store with SQLite.
2. Introduce a shared playback/session layer that both desktop and future Android TV clients can consume.
3. Replace the static frontend with a richer Tauri UI or a bundled frontend framework once the machine has a working Node toolchain.
4. Add authentication, watchlists, progress tracking, and provider integrations.
# sol
