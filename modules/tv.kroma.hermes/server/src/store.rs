use anyhow::Result;
use rusqlite::params;

pub use kroma_module_sdk::db::Pool;

mod audit;
mod inbox;
mod memory;

pub use inbox::{Incoming, State};

pub const MIGRATIONS: &str = "
    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS inbox (
        id         TEXT PRIMARY KEY,
        at         INTEGER NOT NULL,
        user_id    TEXT NOT NULL,
        name       TEXT NOT NULL,
        text       TEXT NOT NULL,
        state      TEXT NOT NULL,
        reason     TEXT,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX IF NOT EXISTS inbox_state ON inbox(state, at);
    CREATE INDEX IF NOT EXISTS inbox_user ON inbox(user_id, updated_at);

    CREATE TABLE IF NOT EXISTS lessons (
        id   INTEGER PRIMARY KEY AUTOINCREMENT,
        at   INTEGER NOT NULL,
        text TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS suggestions (
        id     INTEGER PRIMARY KEY AUTOINCREMENT,
        at     INTEGER NOT NULL,
        kind   TEXT NOT NULL,
        title  TEXT NOT NULL,
        detail TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS person_notes (
        id      INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id TEXT NOT NULL,
        name    TEXT NOT NULL,
        at      INTEGER NOT NULL,
        note    TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS person_notes_user ON person_notes(user_id);

    CREATE TABLE IF NOT EXISTS audit (
        id    INTEGER PRIMARY KEY AUTOINCREMENT,
        at    INTEGER NOT NULL,
        day   INTEGER NOT NULL,
        genre TEXT NOT NULL,
        event TEXT NOT NULL,
        bytes INTEGER NOT NULL,
        data  TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS audit_genre_day ON audit(genre, day);
    CREATE INDEX IF NOT EXISTS audit_at ON audit(at);
";

#[derive(Clone)]
pub struct Store {
    pool: Pool,
}

impl Store {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn kv_get(&self, key: &str) -> Result<Option<String>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT value FROM kv WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |r| r.get::<_, String>(0))?;
        Ok(rows.next().transpose()?)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn kv_delete(&self, key: &str) -> Result<()> {
        self.pool.get()?.execute("DELETE FROM kv WHERE key = ?1", params![key])?;
        Ok(())
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
