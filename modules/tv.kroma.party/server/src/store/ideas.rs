use anyhow::Result;
use rusqlite::{params, Row};
use serde::{Deserialize, Serialize};

use super::Store;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IdeaContext {
    pub channel: Option<String>,
    pub device: Option<String>,
    pub version: Option<String>,
    pub browser: Option<String>,
    pub chat: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Idea {
    pub id: String,
    pub ts: i64,
    pub user_id: String,
    pub name: String,
    pub genre: String,
    pub text: String,
    pub status: String,
    pub reply: Option<String>,
    pub updated_at: i64,
    pub context: Option<IdeaContext>,
    pub source: Option<String>,
}

const COLS: &str = "id, ts, user_id, name, genre, text, status, reply, updated_at, context, source";

fn row(r: &Row) -> rusqlite::Result<Idea> {
    Ok(Idea {
        id: r.get(0)?,
        ts: r.get(1)?,
        user_id: r.get(2)?,
        name: r.get(3)?,
        genre: r.get(4)?,
        text: r.get(5)?,
        status: r.get(6)?,
        reply: r.get(7)?,
        updated_at: r.get(8)?,
        context: r.get::<_, Option<String>>(9)?.and_then(|s| serde_json::from_str(&s).ok()),
        source: r.get(10)?,
    })
}

impl Store {
    pub fn insert_idea(&self, i: &Idea, keep: usize) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            &format!("INSERT INTO ideas ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"),
            params![
                i.id,
                i.ts,
                i.user_id,
                i.name,
                i.genre,
                i.text,
                i.status,
                i.reply,
                i.updated_at,
                i.context.as_ref().map(serde_json::to_string).transpose()?,
                i.source
            ],
        )?;
        conn.execute(
            "DELETE FROM ideas WHERE id NOT IN (SELECT id FROM ideas ORDER BY ts DESC LIMIT ?1)",
            params![keep as i64],
        )?;
        Ok(())
    }

    pub fn ideas(&self, status: Option<&str>, newest_first: bool, limit: u32) -> Result<Vec<Idea>> {
        let conn = self.pool.get()?;
        let order = if newest_first { "DESC" } else { "ASC" };
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM ideas WHERE (?1 IS NULL OR status = ?1) ORDER BY ts {order} LIMIT ?2"
        ))?;
        let rows = stmt.query_map(params![status, i64::from(limit)], row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn idea(&self, id: &str) -> Result<Option<Idea>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM ideas WHERE id = ?1"))?;
        let mut rows = stmt.query_map(params![id], row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn recent_idea_times(&self, user_id: &str, since: i64) -> Result<Vec<i64>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT ts FROM ideas WHERE user_id = ?1 AND ts >= ?2")?;
        let rows = stmt.query_map(params![user_id, since], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn update_idea(&self, id: &str, status: Option<&str>, reply: Option<Option<&str>>, at: i64) -> Result<bool> {
        let conn = self.pool.get()?;
        let mut changed = conn.execute("UPDATE ideas SET updated_at = ?1 WHERE id = ?2", params![at, id])? > 0;
        if let Some(s) = status {
            conn.execute("UPDATE ideas SET status = ?1 WHERE id = ?2", params![s, id])?;
        }
        if let Some(r) = reply {
            changed |= conn.execute("UPDATE ideas SET reply = ?1 WHERE id = ?2", params![r, id])? > 0;
        }
        Ok(changed)
    }
}
