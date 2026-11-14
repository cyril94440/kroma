use anyhow::Result;
use kroma_module_sdk::db::Pool;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;
use rusqlite::{params, OptionalExtension};

use crate::peers::downloads::DownloadRow;

pub const INTENT_FOR_MS: i64 = 5 * 60 * 1000;
pub const GRACE_MS: i64 = 10 * 60 * 1000;

/// A watch-now import placed in the library and not yet confirmed: what it
/// wrote, which is all a rejection may take out, and what it took from the
/// ledger, which is all a rejection gives back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisional {
    pub download_id: String,
    pub request_id: Option<String>,
    pub kind: String,
    pub tmdb_id: u64,
    pub season: Option<u32>,
    pub release_title: String,
    pub paths: Vec<String>,
    pub written: Vec<String>,
    pub wanted_ids: Vec<String>,
    pub created_at: i64,
}

pub fn save(store: &Pool, held: &Provisional) -> Result<()> {
    let conn = store.get()?;
    conn.execute(
        "INSERT OR REPLACE INTO provisional_imports \
         (download_id, request_id, kind, tmdb_id, season, episodes, release_title, paths, written, wanted_ids, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8, ?9, ?10)",
        params![
            held.download_id,
            held.request_id,
            held.kind,
            held.tmdb_id as i64,
            held.season,
            held.release_title,
            serde_json::to_string(&held.paths)?,
            serde_json::to_string(&held.written)?,
            serde_json::to_string(&held.wanted_ids)?,
            held.created_at,
        ],
    )?;
    Ok(())
}

const COLUMNS: &str = "download_id, request_id, kind, tmdb_id, season, release_title, paths, written, wanted_ids, created_at";

fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Provisional> {
    let list = |text: String| serde_json::from_str(&text).unwrap_or_default();
    Ok(Provisional {
        download_id: r.get(0)?,
        request_id: r.get(1)?,
        kind: r.get(2)?,
        tmdb_id: r.get::<_, i64>(3)? as u64,
        season: r.get(4)?,
        release_title: r.get(5)?,
        paths: list(r.get(6)?),
        written: list(r.get(7)?),
        wanted_ids: list(r.get(8)?),
        created_at: r.get(9)?,
    })
}

pub fn get(store: &Pool, download_id: &str) -> Result<Option<Provisional>> {
    let conn = store.get()?;
    Ok(conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM provisional_imports WHERE download_id = ?1"),
            [download_id],
            read,
        )
        .optional()?)
}

pub fn older_than(store: &Pool, cutoff: i64) -> Result<Vec<Provisional>> {
    let conn = store.get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM provisional_imports WHERE created_at < ?1 ORDER BY created_at"
    ))?;
    let rows = stmt.query_map([cutoff], read)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn forget(store: &Pool, download_id: &str) -> Result<()> {
    store.get()?.execute(
        "DELETE FROM provisional_imports WHERE download_id = ?1",
        [download_id],
    )?;
    Ok(())
}

/// The magnets a watch-now is about to offer the download client, noted so the
/// import pass leaves the row they become to the call that made it. Cleared when
/// the guard is dropped; a note older than [`INTENT_FOR_MS`] no longer holds.
pub struct Intents<'a> {
    store: &'a Pool,
    magnets: Vec<String>,
}

impl<'a> Intents<'a> {
    pub fn new(store: &'a Pool) -> Self {
        Self {
            store,
            magnets: Vec::new(),
        }
    }

    pub fn note(&mut self, magnets: impl IntoIterator<Item = String>) {
        let Ok(conn) = self.store.get() else {
            return;
        };
        for magnet in magnets {
            let noted = conn.execute(
                "INSERT OR REPLACE INTO watch_now_intents (magnet_or_url, created_at) VALUES (?1, ?2)",
                params![magnet, now_ms()],
            );
            if noted.is_ok() {
                self.magnets.push(magnet);
            }
        }
    }
}

impl Drop for Intents<'_> {
    fn drop(&mut self) {
        if let Ok(conn) = self.store.get() {
            for magnet in &self.magnets {
                let _ = conn.execute(
                    "DELETE FROM watch_now_intents WHERE magnet_or_url = ?1",
                    [magnet],
                );
            }
        }
    }
}

/// Whether the import pass must leave this download alone: a watch-now placed
/// it and awaits the core's verdict, or is about to.
pub fn is_held_for_watch_now<S: HostStorage>(state: &S, row: &DownloadRow) -> bool {
    let Ok(conn) = state.store().get() else {
        return false;
    };
    let provisional: bool = conn
        .query_row(
            "SELECT 1 FROM provisional_imports WHERE download_id = ?1",
            [&row.id],
            |_| Ok(true),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or(false);
    let intended: bool = conn
        .query_row(
            "SELECT 1 FROM watch_now_intents WHERE magnet_or_url = ?1 AND created_at > ?2",
            params![row.magnet_or_url.trim(), now_ms() - INTENT_FOR_MS],
            |_| Ok(true),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or(false);
    provisional || intended
}
