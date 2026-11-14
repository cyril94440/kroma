use anyhow::Result;
use rusqlite::{params, Row};
use serde_json::Value;

use super::Store;
use crate::model::{ChatMessage, Kind};

const COLS: &str = "id, user_id, name, kind, text, key, params, action, card, reply_to, at, reactions";

fn row(r: &Row) -> rusqlite::Result<ChatMessage> {
    let parse = |s: Option<String>| s.and_then(|s| serde_json::from_str::<Value>(&s).ok());
    Ok(ChatMessage {
        id: r.get(0)?,
        user_id: r.get(1)?,
        name: r.get(2)?,
        kind: Kind::parse(&r.get::<_, String>(3)?),
        text: r.get(4)?,
        key: r.get(5)?,
        params: parse(r.get(6)?)
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default(),
        action: r.get(7)?,
        card: parse(r.get(8)?),
        reply_to: r.get(9)?,
        at: r.get(10)?,
        reactions: parse(r.get(11)?)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
    })
}

impl Store {
    pub fn insert_message(&self, m: &ChatMessage) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            &format!("INSERT INTO chat ({COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"),
            params![
                m.id,
                m.user_id,
                m.name,
                m.kind.as_str(),
                m.text,
                m.key,
                serde_json::to_string(&m.params)?,
                m.action,
                m.card.as_ref().map(Value::to_string),
                m.reply_to,
                m.at,
                serde_json::to_string(&m.reactions)?,
            ],
        )?;
        Ok(())
    }

    pub fn message(&self, id: &str) -> Result<Option<ChatMessage>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM chat WHERE id = ?1"))?;
        let mut rows = stmt.query_map(params![id], row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn recent_messages(&self, limit: u32, before: Option<i64>, since: Option<i64>) -> Result<Vec<ChatMessage>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLS} FROM chat WHERE at < ?1 AND at >= ?2 ORDER BY at DESC, seq DESC LIMIT ?3"
        ))?;
        let rows = stmt.query_map(
            params![before.unwrap_or(i64::MAX), since.unwrap_or(0), i64::from(limit)],
            row,
        )?;
        let mut out: Vec<ChatMessage> = rows.collect::<rusqlite::Result<_>>()?;
        out.reverse();
        Ok(out)
    }

    pub fn within_reaction_window(&self, id: &str, window: usize) -> Result<bool> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT id FROM chat ORDER BY seq DESC LIMIT ?1")?;
        let ids = stmt.query_map(params![window as i64], |r| r.get::<_, String>(0))?;
        for found in ids {
            if found? == id {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn save_reactions(&self, id: &str, m: &ChatMessage) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE chat SET reactions = ?1 WHERE id = ?2",
            params![serde_json::to_string(&m.reactions)?, id],
        )?;
        Ok(())
    }

    pub fn prune_messages(&self, older_than: i64) -> Result<usize> {
        let conn = self.pool.get()?;
        Ok(conn.execute("DELETE FROM chat WHERE at < ?1", params![older_than])?)
    }
}
