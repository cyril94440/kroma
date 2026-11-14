use anyhow::Result;
use rusqlite::params;
use serde::Serialize;

use super::Store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Lesson {
    pub id: i64,
    pub at: i64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub id: i64,
    pub at: i64,
    pub kind: String,
    pub title: String,
    pub detail: String,
}

impl Store {
    pub fn add_lesson(&self, text: &str, at: i64, keep: usize) -> Result<usize> {
        let conn = self.pool.get()?;
        conn.execute("INSERT INTO lessons (at, text) VALUES (?1, ?2)", params![at, text])?;
        conn.execute(
            "DELETE FROM lessons WHERE id NOT IN (SELECT id FROM lessons ORDER BY id DESC LIMIT ?1)",
            params![keep as i64],
        )?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM lessons")?;
        let total = stmt.query_map([], |r| r.get::<_, i64>(0))?.next().transpose()?.unwrap_or(0);
        Ok(total as usize)
    }

    pub fn lessons(&self) -> Result<Vec<Lesson>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT id, at, text FROM lessons ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok(Lesson { id: r.get(0)?, at: r.get(1)?, text: r.get(2)? }))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_lesson(&self, id: i64) -> Result<bool> {
        Ok(self.pool.get()?.execute("DELETE FROM lessons WHERE id = ?1", params![id])? > 0)
    }

    pub fn add_suggestion(&self, kind: &str, title: &str, detail: &str, at: i64, keep: usize) -> Result<usize> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO suggestions (at, kind, title, detail) VALUES (?1, ?2, ?3, ?4)",
            params![at, kind, title, detail],
        )?;
        conn.execute(
            "DELETE FROM suggestions WHERE id NOT IN (SELECT id FROM suggestions ORDER BY id DESC LIMIT ?1)",
            params![keep as i64],
        )?;
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM suggestions")?;
        let total = stmt.query_map([], |r| r.get::<_, i64>(0))?.next().transpose()?.unwrap_or(0);
        Ok(total as usize)
    }

    pub fn suggestions(&self) -> Result<Vec<Suggestion>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT id, at, kind, title, detail FROM suggestions ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Suggestion { id: r.get(0)?, at: r.get(1)?, kind: r.get(2)?, title: r.get(3)?, detail: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_suggestion(&self, id: i64) -> Result<bool> {
        Ok(self.pool.get()?.execute("DELETE FROM suggestions WHERE id = ?1", params![id])? > 0)
    }

    pub fn add_note(&self, user_id: &str, name: &str, note: &str, at: i64, keep: usize) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO person_notes (user_id, name, at, note) VALUES (?1, ?2, ?3, ?4)",
            params![user_id, name, at, note],
        )?;
        conn.execute(
            "DELETE FROM person_notes WHERE user_id = ?1 AND id NOT IN (SELECT id FROM person_notes WHERE user_id = ?1 ORDER BY id DESC LIMIT ?2)",
            params![user_id, keep as i64],
        )?;
        Ok(())
    }

    pub fn notes(&self, user_id: &str) -> Result<Vec<String>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT note FROM person_notes WHERE user_id = ?1 ORDER BY id")?;
        let rows = stmt.query_map(params![user_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn all_notes(&self) -> Result<Vec<(String, String, String)>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare("SELECT user_id, name, note FROM person_notes ORDER BY user_id, id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn forget_person(&self, user_id: &str) -> Result<usize> {
        Ok(self.pool.get()?.execute("DELETE FROM person_notes WHERE user_id = ?1", params![user_id])?)
    }
}
