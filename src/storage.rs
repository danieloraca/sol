use std::{
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::WatchProgressEntry;

#[derive(Clone)]
pub struct WatchProgressStore {
    conn: Arc<Mutex<Connection>>,
}

impl WatchProgressStore {
    pub fn new() -> Result<Self, String> {
        let db_path = resolve_db_path()?;
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create database directory: {error}"))?;
        }

        let connection = Connection::open(&db_path)
            .map_err(|error| format!("Could not open watch progress database: {error}"))?;

        connection
            .execute_batch(
                "
                PRAGMA journal_mode=WAL;
                PRAGMA foreign_keys=ON;
                CREATE TABLE IF NOT EXISTS watch_progress (
                    id TEXT PRIMARY KEY,
                    progress_percent REAL NOT NULL,
                    position_seconds INTEGER NOT NULL,
                    duration_seconds INTEGER NOT NULL,
                    updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_watch_progress_updated_at
                ON watch_progress(updated_at_ms DESC);
                ",
            )
            .map_err(|error| format!("Could not initialize watch progress schema: {error}"))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn list(&self) -> Result<Vec<WatchProgressEntry>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "Watch progress lock poisoned")?;
        let mut statement = conn
            .prepare(
                "
                SELECT id, progress_percent, position_seconds, duration_seconds, updated_at_ms
                FROM watch_progress
                ORDER BY updated_at_ms DESC
                ",
            )
            .map_err(|error| format!("Could not query watch progress: {error}"))?;

        let rows = statement
            .query_map([], |row| {
                Ok(WatchProgressEntry {
                    id: row.get(0)?,
                    progress_percent: row.get(1)?,
                    position_seconds: row.get(2)?,
                    duration_seconds: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            })
            .map_err(|error| format!("Could not read watch progress rows: {error}"))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|error| format!("Invalid watch progress row: {error}"))?);
        }

        Ok(entries)
    }

    pub fn upsert(
        &self,
        id: &str,
        progress_percent: f32,
        position_seconds: u32,
        duration_seconds: u32,
    ) -> Result<(), String> {
        let updated_at_ms = now_unix_ms();
        let conn = self
            .conn
            .lock()
            .map_err(|_| "Watch progress lock poisoned")?;

        conn.execute(
            "
            INSERT INTO watch_progress (
                id,
                progress_percent,
                position_seconds,
                duration_seconds,
                updated_at_ms
            )
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                progress_percent = excluded.progress_percent,
                position_seconds = excluded.position_seconds,
                duration_seconds = excluded.duration_seconds,
                updated_at_ms = excluded.updated_at_ms
            ",
            params![
                id,
                progress_percent,
                i64::from(position_seconds),
                i64::from(duration_seconds),
                updated_at_ms
            ],
        )
        .map_err(|error| format!("Could not save watch progress: {error}"))?;

        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "Watch progress lock poisoned")?;
        conn.execute("DELETE FROM watch_progress WHERE id = ?1", params![id])
            .map_err(|error| format!("Could not delete watch progress: {error}"))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<WatchProgressEntry>, String> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| "Watch progress lock poisoned")?;
        conn.query_row(
            "
            SELECT id, progress_percent, position_seconds, duration_seconds, updated_at_ms
            FROM watch_progress
            WHERE id = ?1
            ",
            params![id],
            |row| {
                Ok(WatchProgressEntry {
                    id: row.get(0)?,
                    progress_percent: row.get(1)?,
                    position_seconds: row.get(2)?,
                    duration_seconds: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                })
            },
        )
        .optional()
        .map_err(|error| format!("Could not load watch progress item: {error}"))
    }
}

fn resolve_db_path() -> Result<PathBuf, String> {
    if let Ok(raw) = env::var("SOL_DB_PATH") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    if let Some(mut data_dir) = dirs::data_local_dir() {
        data_dir.push("sol");
        data_dir.push("sol.sqlite3");
        return Ok(data_dir);
    }

    let mut fallback =
        env::current_dir().map_err(|error| format!("Could not resolve cwd: {error}"))?;
    fallback.push("sol.sqlite3");
    Ok(fallback)
}

fn now_unix_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as i64,
        Err(_) => 0,
    }
}
