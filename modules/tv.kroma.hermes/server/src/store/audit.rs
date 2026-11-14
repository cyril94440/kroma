use anyhow::Result;
use rusqlite::params;
use serde::Serialize;
use serde_json::Value;

use super::Store;

pub const LINE_MAX_BYTES: usize = 64 * 1024;
pub const GENRE_DAY_MAX_BYTES: i64 = 40 * 1024 * 1024;
const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AuditRow {
    pub id: i64,
    pub at: i64,
    pub genre: String,
    pub event: String,
    pub data: Value,
}

fn valid_genre(genre: &str) -> bool {
    !genre.is_empty() && genre.len() <= 32 && genre.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

impl Store {
    pub fn record(&self, genre: &str, event: &str, data: &Value, at: i64) -> bool {
        self.try_record(genre, event, data, at).unwrap_or(false)
    }

    fn try_record(&self, genre: &str, event: &str, data: &Value, at: i64) -> Result<bool> {
        if !valid_genre(genre) {
            return Ok(false);
        }
        let mut line = data.to_string();
        if line.len() > LINE_MAX_BYTES {
            let head: String = line.chars().take(LINE_MAX_BYTES / 2).collect();
            line = serde_json::json!({ "truncated": line.len(), "head": head }).to_string();
        }
        let day = at.div_euclid(DAY_MS);
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT COALESCE(SUM(bytes), 0) FROM audit WHERE genre = ?1 AND day = ?2")?;
        let used = stmt.query_map(params![genre, day], |r| r.get::<_, i64>(0))?.next().transpose()?.unwrap_or(0);
        if used + line.len() as i64 > GENRE_DAY_MAX_BYTES {
            return Ok(false);
        }
        conn.execute(
            "INSERT INTO audit (at, day, genre, event, bytes, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![at, day, genre, event, line.len() as i64, line],
        )?;
        Ok(true)
    }

    pub fn audit_rows(&self, genre: Option<&str>, before: Option<i64>, limit: u32) -> Result<Vec<AuditRow>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, at, genre, event, data FROM audit WHERE (?1 IS NULL OR genre = ?1) AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![genre, before, limit.clamp(1, 500)], |r| {
            let data: String = r.get(4)?;
            Ok(AuditRow {
                id: r.get(0)?,
                at: r.get(1)?,
                genre: r.get(2)?,
                event: r.get(3)?,
                data: serde_json::from_str(&data).unwrap_or(Value::Null),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn audit_genres(&self) -> Result<Vec<String>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT DISTINCT genre FROM audit ORDER BY genre")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn audit_count_since(&self, genre: &str, event: &str, since: i64) -> Result<u32> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM audit WHERE genre = ?1 AND event = ?2 AND at >= ?3")?;
        let total = stmt.query_map(params![genre, event, since], |r| r.get::<_, i64>(0))?.next().transpose()?.unwrap_or(0);
        Ok(total as u32)
    }

    pub fn prune_audit(&self, before: i64) -> Result<usize> {
        Ok(self.pool.get()?.execute("DELETE FROM audit WHERE at < ?1", params![before])?)
    }
}
