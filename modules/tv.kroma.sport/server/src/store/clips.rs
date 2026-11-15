use std::collections::BTreeMap;

use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Store;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClipRow {
    pub id: String,
    pub match_id: String,
    pub ev_id: String,
    pub dom: String,
    pub ext: String,
    pub competition: String,
    pub minute: String,
    pub equipe: String,
    pub buteur: String,
    pub score: String,
    pub debut_ms: i64,
    pub fin_ms: i64,
    pub duree_ms: i64,
    pub path: String,
    pub ts: i64,
}

pub type NewClip = ClipRow;

const COLUMNS: &str = "id, match_id, ev_id, home, away, competition, minute, side, scorer, score, from_ms, to_ms, duration_ms, path, created_at";

fn row(r: &rusqlite::Row) -> rusqlite::Result<ClipRow> {
    Ok(ClipRow {
        id: r.get(0)?,
        match_id: r.get(1)?,
        ev_id: r.get(2)?,
        dom: r.get(3)?,
        ext: r.get(4)?,
        competition: r.get(5)?,
        minute: r.get(6)?,
        equipe: r.get(7)?,
        buteur: r.get(8)?,
        score: r.get(9)?,
        debut_ms: r.get(10)?,
        fin_ms: r.get(11)?,
        duree_ms: r.get(12)?,
        path: r.get(13)?,
        ts: r.get(14)?,
    })
}

impl Store {
    pub fn clip_insert(&self, clip: &NewClip) -> Result<()> {
        let conn = self.pool().get()?;
        conn.execute(
            &format!("INSERT OR REPLACE INTO clips ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"),
            params![clip.id, clip.match_id, clip.ev_id, clip.dom, clip.ext, clip.competition, clip.minute, clip.equipe, clip.buteur, clip.score, clip.debut_ms, clip.fin_ms, clip.duree_ms, clip.path, clip.ts],
        )?;
        Ok(())
    }

    pub fn clips(&self, match_id: Option<&str>, limit: u32) -> Result<Vec<ClipRow>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM clips WHERE (?1 IS NULL OR match_id = ?1) ORDER BY to_ms DESC LIMIT ?2"))?;
        let rows = stmt
            .query_map(params![match_id, limit], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn clip(&self, id: &str) -> Result<Option<ClipRow>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM clips WHERE id = ?1"))?;
        let mut rows = stmt.query_map(params![id], row)?;
        Ok(rows.next().transpose()?)
    }

    pub fn clip_delete(&self, id: &str) -> Result<()> {
        self.pool()
            .get()?
            .execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clips_older_than(&self, before_ms: i64) -> Result<Vec<ClipRow>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM clips WHERE to_ms < ?1 ORDER BY to_ms"
        ))?;
        let rows = stmt
            .query_map(params![before_ms], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn clips_over(&self, keep: u32) -> Result<Vec<ClipRow>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM clips ORDER BY to_ms DESC LIMIT -1 OFFSET ?1"
        ))?;
        let rows = stmt
            .query_map(params![keep], row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn attempts(&self) -> Result<BTreeMap<String, u32>> {
        let conn = self.pool().get()?;
        let mut stmt = conn.prepare("SELECT id, attempts FROM clip_attempts")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows.into_iter().collect())
    }

    pub fn attempt_add(&self, id: &str, now_ms: i64) -> Result<()> {
        self.pool().get()?.execute(
            "INSERT INTO clip_attempts (id, attempts, updated_at) VALUES (?1, 1, ?2) ON CONFLICT(id) DO UPDATE SET attempts = attempts + 1, updated_at = excluded.updated_at",
            params![id, now_ms],
        )?;
        Ok(())
    }

    pub fn attempts_prune(&self, before_ms: i64) -> Result<()> {
        self.pool().get()?.execute(
            "DELETE FROM clip_attempts WHERE updated_at < ?1",
            params![before_ms],
        )?;
        Ok(())
    }
}
