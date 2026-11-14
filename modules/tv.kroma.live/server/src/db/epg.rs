use anyhow::Result;
use rusqlite::params;

use super::Pool;

const BATCH: usize = 150;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProgramme {
    pub epg_id: String,
    pub start: i64,
    pub stop: i64,
    pub title: String,
    pub hay: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpgState {
    pub fetched_at: i64,
    pub programmes: i64,
    pub source: String,
    pub error: Option<String>,
}

pub fn replace_programmes(pool: &Pool, rows: &[StoredProgramme], source: &str, now_ms: i64) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM programmes", [])?;
    for chunk in rows.chunks(BATCH) {
        let marks = vec!["(?,?,?,?,?,?)"; chunk.len()].join(",");
        let mut values: Vec<rusqlite::types::Value> = Vec::with_capacity(chunk.len() * 6);
        for row in chunk {
            values.extend([row.epg_id.clone().into(), row.start.into(), row.stop.into(), row.title.clone().into(), row.hay.clone().into(), row.description.clone().into()]);
        }
        tx.execute(&format!("INSERT INTO programmes (epg_id, start, stop, title, hay, descr) VALUES {marks}"), rusqlite::params_from_iter(values))?;
    }
    tx.execute(
        "INSERT INTO epg_state (id, fetched_at, programmes, source, error) VALUES (1, ?1, ?2, ?3, NULL) \
         ON CONFLICT(id) DO UPDATE SET fetched_at = excluded.fetched_at, programmes = excluded.programmes, source = excluded.source, error = NULL",
        params![now_ms, rows.len() as i64, source],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn note_epg_failure(pool: &Pool, error: &str) -> Result<()> {
    pool.get()?.execute(
        "INSERT INTO epg_state (id, fetched_at, programmes, source, error) VALUES (1, 0, 0, '', ?1) \
         ON CONFLICT(id) DO UPDATE SET error = excluded.error",
        params![error],
    )?;
    Ok(())
}

pub fn epg_state(pool: &Pool) -> Result<EpgState> {
    let conn = pool.get()?;
    match conn.query_row("SELECT fetched_at, programmes, source, error FROM epg_state WHERE id = 1", [], |r| {
        Ok(EpgState { fetched_at: r.get(0)?, programmes: r.get(1)?, source: r.get(2)?, error: r.get(3)? })
    }) {
        Ok(state) => Ok(state),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(EpgState::default()),
        Err(error) => Err(error.into()),
    }
}

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoredProgramme> {
    Ok(StoredProgramme { epg_id: r.get(0)?, start: r.get(1)?, stop: r.get(2)?, title: r.get(3)?, hay: r.get(4)?, description: r.get(5)? })
}

const COLUMNS: &str = "epg_id, start, stop, title, hay, descr";

pub fn programmes_of(pool: &Pool, epg_id: &str, from: i64, to: i64) -> Result<Vec<StoredProgramme>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM programmes WHERE epg_id = ?1 AND stop > ?2 AND start < ?3 ORDER BY start"))?;
    let rows = stmt.query_map(params![epg_id.to_lowercase(), from, to], row)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn programmes_window(pool: &Pool, from: i64, to: i64, limit: usize) -> Result<Vec<StoredProgramme>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM programmes WHERE stop > ?1 AND start < ?2 ORDER BY start, epg_id LIMIT ?3"))?;
    let rows = stmt.query_map(params![from, to, limit as i64], row)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn search_programmes(pool: &Pool, tokens: &[String], any: bool, from: i64, limit: usize) -> Result<Vec<StoredProgramme>> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }
    let joiner = if any { " OR " } else { " AND " };
    let clause = vec!["hay LIKE ? ESCAPE '\\'"; tokens.len()].join(joiner);
    let mut values: Vec<rusqlite::types::Value> = vec![from.into()];
    values.extend(tokens.iter().map(|t| format!("%{}%", t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")).into()));
    values.push((limit as i64).into());
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM programmes WHERE stop >= ?1 AND ({clause}) ORDER BY start LIMIT ?{}", tokens.len() + 2))?;
    let rows = stmt.query_map(rusqlite::params_from_iter(values), row)?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    fn programme(epg: &str, start: i64, stop: i64, title: &str) -> StoredProgramme {
        StoredProgramme { epg_id: epg.into(), start, stop, title: title.into(), hay: title.to_lowercase(), description: String::new() }
    }

    #[test]
    fn programmes_are_read_by_channel_and_window_in_start_order() {
        let pool = test_pool();
        replace_programmes(&pool, &[programme("tf1.fr", 200, 300, "B"), programme("tf1.fr", 100, 200, "A"), programme("m6.fr", 100, 200, "Z")], "primary", 5).unwrap();

        let found = programmes_of(&pool, "TF1.fr", 150, 250).unwrap();

        assert_eq!(found.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(epg_state(&pool).unwrap(), EpgState { fetched_at: 5, programmes: 3, source: "primary".into(), error: None });
    }

    #[test]
    fn a_search_needs_every_token_or_any_and_escapes_wildcards() {
        let pool = test_pool();
        replace_programmes(&pool, &[programme("a", 0, 100, "psg liverpool"), programme("b", 0, 100, "psg nantes"), programme("c", 0, 100, "100% sport")], "p", 1).unwrap();

        assert_eq!(search_programmes(&pool, &["psg".into(), "liverpool".into()], false, 0, 10).unwrap().len(), 1);
        assert_eq!(search_programmes(&pool, &["liverpool".into(), "nantes".into()], true, 0, 10).unwrap().len(), 2);
        assert_eq!(search_programmes(&pool, &["%".into()], false, 0, 10).unwrap().len(), 1);
        assert!(search_programmes(&pool, &["psg".into()], false, 101, 10).unwrap().is_empty());
    }

    #[test]
    fn a_failed_refresh_keeps_the_programmes_and_says_why() {
        let pool = test_pool();
        replace_programmes(&pool, &[programme("a", 0, 100, "x")], "p", 1).unwrap();

        note_epg_failure(&pool, "HTTP 500").unwrap();

        assert_eq!(epg_state(&pool).unwrap().error.as_deref(), Some("HTTP 500"));
        assert_eq!(programmes_window(&pool, 0, 100, 10).unwrap().len(), 1);
    }
}
