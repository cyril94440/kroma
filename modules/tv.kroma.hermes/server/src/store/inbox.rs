use anyhow::Result;
use rusqlite::{params, Row};

use super::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Queued,
    Active,
    Done,
    Skipped,
    Interrupted,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Queued => "queued",
            State::Active => "active",
            State::Done => "done",
            State::Skipped => "skipped",
            State::Interrupted => "interrupted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incoming {
    pub id: String,
    pub at: i64,
    pub user_id: String,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxRow {
    pub message: Incoming,
    pub state: String,
    pub reason: Option<String>,
}

fn row(r: &Row) -> rusqlite::Result<InboxRow> {
    Ok(InboxRow {
        message: Incoming { id: r.get(0)?, at: r.get(1)?, user_id: r.get(2)?, name: r.get(3)?, text: r.get(4)? },
        state: r.get(5)?,
        reason: r.get(6)?,
    })
}

const COLS: &str = "id, at, user_id, name, text, state, reason";

impl Store {
    pub fn claim(&self, m: &Incoming, state: State, reason: Option<&str>, now: i64) -> Result<bool> {
        let conn = self.pool.get()?;
        let added = conn.execute(
            "INSERT OR IGNORE INTO inbox (id, at, user_id, name, text, state, reason, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![m.id, m.at, m.user_id, m.name, m.text, state.as_str(), reason, now],
        )?;
        Ok(added == 1)
    }

    pub fn next_queued(&self) -> Result<Option<InboxRow>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM inbox WHERE state = 'queued' ORDER BY at, id LIMIT 1"))?;
        let mut rows = stmt.query_map([], row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn set_state(&self, id: &str, state: State, reason: Option<&str>, now: i64) -> Result<()> {
        self.pool.get()?.execute(
            "UPDATE inbox SET state = ?2, reason = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, state.as_str(), reason, now],
        )?;
        Ok(())
    }

    pub fn interrupt_active(&self, now: i64) -> Result<usize> {
        Ok(self.pool.get()?.execute(
            "UPDATE inbox SET state = 'interrupted', reason = 'restarted', updated_at = ?1 WHERE state = 'active'",
            params![now],
        )?)
    }

    pub fn engaged_since(&self, user_id: &str, since: i64) -> Result<u32> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM inbox WHERE user_id = ?1 AND updated_at >= ?2 AND state IN ('active', 'done', 'interrupted')",
        )?;
        let mut rows = stmt.query_map(params![user_id, since], |r| r.get::<_, i64>(0))?;
        Ok(rows.next().transpose()?.unwrap_or(0) as u32)
    }

    pub fn inbox_recent(&self, limit: u32) -> Result<Vec<InboxRow>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM inbox ORDER BY at DESC, id DESC LIMIT ?1"))?;
        let rows = stmt.query_map(params![limit], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn prune_inbox(&self, before: i64) -> Result<usize> {
        Ok(self.pool.get()?.execute("DELETE FROM inbox WHERE updated_at < ?1 AND state != 'queued'", params![before])?)
    }
}
