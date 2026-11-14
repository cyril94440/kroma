pub use kroma_module_sdk::db::*;

mod catalogue;
mod epg;
mod history;
mod kv;

pub use catalogue::*;
pub use epg::*;
pub use history::*;
pub use kv::*;

pub const MIGRATIONS: &str = "
    CREATE TABLE IF NOT EXISTS channels (
        provider  TEXT NOT NULL,
        stream_id INTEGER NOT NULL,
        name      TEXT NOT NULL,
        epg_id    TEXT,
        grp       TEXT NOT NULL DEFAULT '',
        url       TEXT,
        PRIMARY KEY (provider, stream_id)
    );
    CREATE TABLE IF NOT EXISTS catalogue_state (
        provider   TEXT PRIMARY KEY,
        fetched_at INTEGER NOT NULL,
        channels   INTEGER NOT NULL,
        error      TEXT
    );
    CREATE TABLE IF NOT EXISTS programmes (
        epg_id TEXT NOT NULL,
        start  INTEGER NOT NULL,
        stop   INTEGER NOT NULL,
        title  TEXT NOT NULL,
        hay    TEXT NOT NULL,
        descr  TEXT NOT NULL DEFAULT ''
    );
    CREATE INDEX IF NOT EXISTS idx_programmes_channel ON programmes(epg_id, start);
    CREATE TABLE IF NOT EXISTS epg_state (
        id         INTEGER PRIMARY KEY CHECK (id = 1),
        fetched_at INTEGER NOT NULL,
        programmes INTEGER NOT NULL,
        source     TEXT NOT NULL DEFAULT '',
        error      TEXT
    );
    CREATE TABLE IF NOT EXISTS history (
        at       INTEGER PRIMARY KEY,
        kbps     INTEGER NOT NULL,
        channel  TEXT,
        provider TEXT,
        health   TEXT,
        loss     REAL NOT NULL DEFAULT 0,
        frozen   INTEGER NOT NULL DEFAULT 0,
        black    INTEGER NOT NULL DEFAULT 0,
        restarts INTEGER NOT NULL DEFAULT 0,
        paused   INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS provider_events (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        at            INTEGER NOT NULL,
        kind          TEXT NOT NULL,
        from_provider TEXT,
        to_provider   TEXT,
        channel       TEXT,
        detail        TEXT NOT NULL DEFAULT ''
    );
    CREATE TABLE IF NOT EXISTS zaps (
        at             INTEGER PRIMARY KEY,
        total_ms       INTEGER NOT NULL,
        probe_ms       INTEGER NOT NULL,
        stop_ms        INTEGER NOT NULL,
        launch_ms      INTEGER NOT NULL,
        publication_ms INTEGER NOT NULL
    );
    CREATE TABLE IF NOT EXISTS kv (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
";

#[cfg(test)]
pub(crate) fn test_pool() -> kroma_module_sdk::db::testing::TempPool {
    let pool = kroma_module_sdk::db::testing::temp_pool("live-test");
    {
        let conn = pool.get().unwrap();
        apply_migrations(&conn, MIGRATIONS).unwrap();
    }
    pool
}
