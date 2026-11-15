use anyhow::Result;
use rusqlite::params;

pub use kroma_module_sdk::db::Pool;

mod clips;
mod trace;

pub use clips::{ClipRow, NewClip};
pub use trace::TraceRow;

pub const MIGRATIONS: &str = "
    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS clips (
        id          TEXT PRIMARY KEY,
        match_id    TEXT NOT NULL,
        ev_id       TEXT NOT NULL,
        home        TEXT NOT NULL,
        away        TEXT NOT NULL,
        competition TEXT NOT NULL,
        minute      TEXT NOT NULL,
        side        TEXT NOT NULL,
        scorer      TEXT NOT NULL,
        score       TEXT NOT NULL,
        from_ms     INTEGER NOT NULL,
        to_ms       INTEGER NOT NULL,
        duration_ms INTEGER NOT NULL,
        path        TEXT NOT NULL,
        created_at  INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS clips_to ON clips(to_ms DESC);
    CREATE INDEX IF NOT EXISTS clips_match ON clips(match_id);

    CREATE TABLE IF NOT EXISTS clip_attempts (
        id         TEXT PRIMARY KEY,
        attempts   INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS trace (
        id    INTEGER PRIMARY KEY AUTOINCREMENT,
        at    INTEGER NOT NULL,
        day   INTEGER NOT NULL,
        genre TEXT NOT NULL,
        event TEXT NOT NULL,
        bytes INTEGER NOT NULL,
        data  TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS trace_genre_day ON trace(genre, day);
    CREATE INDEX IF NOT EXISTS trace_at ON trace(at);
";

#[derive(Clone)]
pub struct Store {
    pool: Pool,
}

impl Store {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub(crate) fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT value FROM kv WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |r| r.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute("INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
        Ok(())
    }

    pub fn kv_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        serde_json::from_str(&self.kv_get(key).ok().flatten()?).ok()
    }

    pub fn kv_put<T: serde::Serialize>(&self, key: &str, value: &T) {
        if let Ok(text) = serde_json::to_string(value) {
            if let Err(e) = self.kv_set(key, &text) {
                tracing::warn!(error = %e, key, "sport state not saved");
            }
        }
    }
}

#[cfg(test)]
pub fn test_store(tag: &str) -> (Store, kroma_module_sdk::db::testing::TempPool) {
    let temp = kroma_module_sdk::db::testing::temp_pool(tag);
    temp.get().unwrap().execute_batch(MIGRATIONS).unwrap();
    (Store::new((*temp).clone()), temp)
}

#[cfg(test)]
mod tests;
