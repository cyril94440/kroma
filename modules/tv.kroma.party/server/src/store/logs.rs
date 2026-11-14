use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Store;

pub const LOG_KEEP: i64 = 4000;
pub const LOG_RETENTION_MS: i64 = 72 * 3600 * 1000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub at: i64,
    pub user_id: String,
    pub name: String,
    pub device: Option<String>,
    pub category: String,
    pub event: String,
    pub data: Option<serde_json::Value>,
}

impl Store {
    pub fn insert_logs(&self, entries: &[LogEntry]) -> Result<()> {
        let conn = self.pool.get()?;
        for e in entries {
            conn.execute(
                "INSERT INTO client_logs (at, user_id, name, device, category, event, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    e.at,
                    e.user_id,
                    e.name,
                    e.device,
                    e.category,
                    e.event,
                    e.data.as_ref().map(serde_json::Value::to_string)
                ],
            )?;
        }
        Ok(())
    }

    pub fn logs(&self, since: i64, user_id: Option<&str>, category: Option<&str>, limit: u32) -> Result<Vec<LogEntry>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT at, user_id, name, device, category, event, data FROM client_logs \
             WHERE at >= ?1 AND (?2 IS NULL OR user_id = ?2) AND (?3 IS NULL OR category = ?3) \
             ORDER BY at DESC, id DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![since, user_id, category, i64::from(limit)], |r| {
            Ok(LogEntry {
                at: r.get(0)?,
                user_id: r.get(1)?,
                name: r.get(2)?,
                device: r.get(3)?,
                category: r.get(4)?,
                event: r.get(5)?,
                data: r.get::<_, Option<String>>(6)?.and_then(|s| serde_json::from_str(&s).ok()),
            })
        })?;
        let mut out: Vec<LogEntry> = rows.collect::<rusqlite::Result<_>>()?;
        out.reverse();
        Ok(out)
    }

    pub fn prune_logs(&self, now: i64) -> Result<usize> {
        let conn = self.pool.get()?;
        let mut removed = conn.execute("DELETE FROM client_logs WHERE at < ?1", params![now - LOG_RETENTION_MS])?;
        removed += conn.execute(
            "DELETE FROM client_logs WHERE id NOT IN (SELECT id FROM client_logs ORDER BY id DESC LIMIT ?1)",
            params![LOG_KEEP],
        )?;
        Ok(removed)
    }
}
