//! What ffprobe found, and the representative columns it recomputes.

use anyhow::Result;
use rusqlite::params;

use crate::Pool;

/// The version of the facts a probe writes. A release that makes the probe learn
/// something new raises it, and every file whose facts are older and could gain
/// from it is read again in the background while it keeps serving the old ones.
pub const PROBE_SCHEMA: i64 = 1;

type FileToProbe = (String, String, String);

fn files_where(pool: &Pool, condition: &str, limit: i64) -> Result<Vec<FileToProbe>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT id, abs_path, item_id FROM files WHERE {condition} \
         ORDER BY v_hdr DESC, id LIMIT ?1"
    ))?;
    let rows = stmt.query_map(params![limit], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// (file_id, abs_path, owning item_id) for every file awaiting an ffprobe pass.
pub fn unprobed_files(pool: &Pool) -> Result<Vec<FileToProbe>> {
    files_where(pool, "probed = 0", -1)
}

/// Up to `limit` files that are probed and readable but were read by an older
/// probe, HDR ones first. They are as probed as any other until the new facts
/// land: every reader keeps using the ones they have.
pub fn stale_probe_files(pool: &Pool, limit: usize) -> Result<Vec<FileToProbe>> {
    files_where(
        pool,
        &format!("probed = 1 AND unreadable IS NULL AND probe_schema < {PROBE_SCHEMA}"),
        i64::try_from(limit).unwrap_or(i64::MAX),
    )
}

/// How many files [`stale_probe_files`] has left.
pub fn stale_probe_count(pool: &Pool) -> Result<usize> {
    let conn = pool.get()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE probed = 1 AND unreadable IS NULL AND probe_schema < ?1",
        params![PROBE_SCHEMA],
        |r| r.get(0),
    )?;
    Ok(n as usize)
}

/// Settle a file whose re-read failed with the facts it already has, so a
/// transient fault does not replace good ones with an error and the file is not
/// offered again until the probe changes once more.
pub fn keep_probe_facts(pool: &Pool, file_id: &str) -> Result<()> {
    pool.get()?.execute(
        "UPDATE files SET probe_schema = ?2 WHERE id = ?1",
        params![file_id, PROBE_SCHEMA],
    )?;
    Ok(())
}

/// Item ids that are fully probed (≥1 file, none unprobed).
pub fn probed_item_ids(pool: &Pool) -> Result<std::collections::HashSet<String>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT item_id FROM files GROUP BY item_id \
         HAVING SUM(CASE WHEN probed=0 THEN 1 ELSE 0 END)=0",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Whether every file of an item is probed (and it has at least one).
pub fn item_probed(pool: &Pool, item_id: &str) -> Result<bool> {
    let conn = pool.get()?;
    let (total, unprobed): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), SUM(CASE WHEN probed=0 THEN 1 ELSE 0 END) FROM files WHERE item_id=?1",
        params![item_id],
        |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0))),
    )?;
    Ok(total > 0 && unprobed == 0)
}

pub fn file_ids_for_item(pool: &Pool, item_id: &str) -> Result<Vec<String>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id FROM files WHERE item_id=?1")?;
    let rows = stmt.query_map(params![item_id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn unprobe_item_files(pool: &Pool, item_id: &str) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "UPDATE files SET probed=0 WHERE item_id=?1",
        params![item_id],
    )?;
    Ok(())
}

/// Every file's `(id, "mtime:size")`: a changed file gets a new signature and is
/// re-probed.
pub fn all_file_sigs(pool: &Pool) -> Result<Vec<(String, String)>> {
    let conn = pool.get()?;
    let mut stmt =
        conn.prepare("SELECT id, COALESCE(mtime,0) || ':' || COALESCE(size,0) FROM files")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// One file as a probe pass sees it. `described` is false for a file nothing has
/// read yet AND for one whose last pass came back with a fault: a mount that was
/// away or an ffprobe the box killed must not leave a title with no streams for
/// good.
pub struct ProbeTarget {
    pub abs_path: String,
    pub item_id: String,
    pub described: bool,
}

/// `None` if the file row is gone.
pub fn probe_target(pool: &Pool, file_id: &str) -> Result<Option<ProbeTarget>> {
    let conn = pool.get()?;
    let mut stmt =
        conn.prepare("SELECT abs_path, item_id, probed, unreadable FROM files WHERE id=?1")?;
    let mut rows = stmt.query_map(params![file_id], |r| {
        Ok(ProbeTarget {
            abs_path: r.get(0)?,
            item_id: r.get(1)?,
            described: r.get::<_, i64>(2)? != 0 && r.get::<_, Option<String>>(3)?.is_none(),
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn item_has_probed_file(pool: &Pool, item_id: &str) -> Result<bool> {
    let conn = pool.get()?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE item_id = ?1 AND probed = 1",
        params![item_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

#[cfg(test)]
mod tests;
