use anyhow::Result;
use rusqlite::params;

use super::Store;

pub struct VoteRecord<'a> {
    pub vote_id: u64,
    pub kind: &'a str,
    pub label: &'a str,
    pub requester_id: Option<&'a str>,
    pub requester_name: Option<&'a str>,
    pub yes: u32,
    pub no: u32,
    pub blank: u32,
    pub verdict: &'a str,
    pub opened_at: i64,
    pub closed_at: i64,
}

impl Store {
    pub fn record_vote(&self, v: &VoteRecord) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO vote_history (vote_id, kind, label, requester_id, requester_name, yes, no, blank, verdict, opened_at, closed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                v.vote_id as i64,
                v.kind,
                v.label,
                v.requester_id,
                v.requester_name,
                v.yes,
                v.no,
                v.blank,
                v.verdict,
                v.opened_at,
                v.closed_at
            ],
        )?;
        Ok(())
    }

    pub fn recent_votes(&self, limit: u32) -> Result<Vec<serde_json::Value>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT vote_id, kind, label, requester_name, yes, no, blank, verdict, closed_at \
             FROM vote_history ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![i64::from(limit)], |r| {
            Ok(serde_json::json!({
                "id": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "label": r.get::<_, String>(2)?,
                "by": r.get::<_, Option<String>>(3)?, "yes": r.get::<_, i64>(4)?, "no": r.get::<_, i64>(5)?,
                "blank": r.get::<_, i64>(6)?, "verdict": r.get::<_, String>(7)?, "closedAt": r.get::<_, i64>(8)?,
            }))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn record_poll(&self, poll_id: u64, question: &str, kind: &str, results: &str, reason: &str, at: i64) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO poll_history (poll_id, question, kind, results, reason, closed_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![poll_id as i64, question, kind, results, reason, at],
        )?;
        Ok(())
    }
}
