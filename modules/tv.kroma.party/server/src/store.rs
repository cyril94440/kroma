use anyhow::Result;
use rusqlite::params;

pub use kroma_module_sdk::db::Pool;

mod chat;
mod history;
mod ideas;
mod logs;

pub use history::VoteRecord;
pub use ideas::{Idea, IdeaContext};
pub use logs::LogEntry;

pub const MIGRATIONS: &str = "
    CREATE TABLE IF NOT EXISTS chat (
        seq       INTEGER PRIMARY KEY AUTOINCREMENT,
        id        TEXT NOT NULL UNIQUE,
        user_id   TEXT,
        name      TEXT NOT NULL,
        kind      TEXT NOT NULL,
        text      TEXT NOT NULL,
        key       TEXT,
        params    TEXT NOT NULL DEFAULT '{}',
        action    TEXT,
        card      TEXT,
        reply_to  TEXT,
        at        INTEGER NOT NULL,
        reactions TEXT NOT NULL DEFAULT '{}'
    );
    CREATE INDEX IF NOT EXISTS chat_at ON chat(at);

    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS vote_history (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        vote_id        INTEGER NOT NULL,
        kind           TEXT NOT NULL,
        label          TEXT NOT NULL,
        requester_id   TEXT,
        requester_name TEXT,
        yes            INTEGER NOT NULL,
        no             INTEGER NOT NULL,
        blank          INTEGER NOT NULL,
        verdict        TEXT NOT NULL,
        opened_at      INTEGER NOT NULL,
        closed_at      INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS poll_history (
        id        INTEGER PRIMARY KEY AUTOINCREMENT,
        poll_id   INTEGER NOT NULL,
        question  TEXT NOT NULL,
        kind      TEXT NOT NULL,
        results   TEXT NOT NULL,
        reason    TEXT NOT NULL,
        closed_at INTEGER NOT NULL
    );

    CREATE TABLE IF NOT EXISTS ideas (
        id         TEXT PRIMARY KEY,
        ts         INTEGER NOT NULL,
        user_id    TEXT NOT NULL,
        name       TEXT NOT NULL,
        genre      TEXT NOT NULL,
        text       TEXT NOT NULL,
        status     TEXT NOT NULL DEFAULT 'received',
        reply      TEXT,
        updated_at INTEGER NOT NULL,
        context    TEXT,
        source     TEXT
    );

    CREATE TABLE IF NOT EXISTS client_logs (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        at       INTEGER NOT NULL,
        user_id  TEXT NOT NULL,
        name     TEXT NOT NULL,
        device   TEXT,
        category TEXT NOT NULL,
        event    TEXT NOT NULL,
        data     TEXT
    );
    CREATE INDEX IF NOT EXISTS client_logs_at ON client_logs(at);
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
            "INSERT INTO kv (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
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
