use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;

use super::Store;

const LINE_MAX: usize = 64 * 1024;
const DAY_MAX_BYTES: i64 = 40 * 1024 * 1024;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TraceRow {
    pub id: i64,
    pub at: i64,
    pub genre: String,
    pub event: String,
    pub data: Value,
}

impl Store {
    pub fn trace(&self, now_ms: i64, genre: &str, event: &str, data: &Value) {
        let mut text = data.to_string();
        if text.len() > LINE_MAX {
            text = serde_json::json!({"tronque": true, "debut": text.chars().take(LINE_MAX / 2).collect::<String>()}).to_string();
        }
        let day = now_ms / DAY_MS;
        let write = || -> Result<()> {
            let conn = self.pool().get()?;
            let used: i64 = conn.query_row(
                "SELECT COALESCE(SUM(bytes), 0) FROM trace WHERE genre = ?1 AND day = ?2",
                params![genre, day],
                |r| r.get(0),
            )?;
            if used < DAY_MAX_BYTES {
                conn.execute("INSERT INTO trace (at, day, genre, event, bytes, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![now_ms, day, genre, event, text.len() as i64, text])?;
            }
            Ok(())
        };
        if let Err(e) = write() {
            tracing::warn!(error = %e, "sport trace not written");
        }
    }

    pub fn trace_rows(
        &self,
        genre: Option<&str>,
        before_id: Option<i64>,
        limit: u32,
    ) -> Result<Vec<TraceRow>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare("SELECT id, at, genre, event, data FROM trace WHERE (?1 IS NULL OR genre = ?1) AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3")?;
        let rows = stmt
            .query_map(params![genre, before_id, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, at, genre, event, data)| TraceRow {
                id,
                at,
                genre,
                event,
                data: serde_json::from_str(&data).unwrap_or(Value::Null),
            })
            .collect())
    }

    pub fn trace_prune(&self, before_ms: i64) -> Result<usize> {
        Ok(self
            .pool()
            .get()?
            .execute("DELETE FROM trace WHERE at < ?1", params![before_ms])?)
    }
}
