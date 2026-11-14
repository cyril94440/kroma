use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Pool;

pub const STEP_SECONDS: i64 = 10;
pub const KEPT_SECONDS: i64 = 3 * 3600;
pub const MAX_MINUTES: i64 = 180;
const ZAPS_KEPT: i64 = 50;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryPoint {
    pub at: i64,
    pub kbps: i64,
    pub channel: Option<String>,
    pub provider: Option<String>,
    pub health: Option<String>,
    pub loss: f64,
    pub frozen: bool,
    pub black: bool,
    pub restarts: i64,
    pub paused: bool,
}

pub fn add_history_point(pool: &Pool, point: &HistoryPoint) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT OR REPLACE INTO history (at, kbps, channel, provider, health, loss, frozen, black, restarts, paused) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![point.at, point.kbps, point.channel, point.provider, point.health, point.loss, point.frozen, point.black, point.restarts, point.paused],
    )?;
    conn.execute("DELETE FROM history WHERE at < ?1", params![point.at - KEPT_SECONDS])?;
    Ok(())
}

pub fn history_since(pool: &Pool, minutes: i64, now_s: i64) -> Result<Vec<HistoryPoint>> {
    let minutes = minutes.clamp(1, MAX_MINUTES);
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT at, kbps, channel, provider, health, loss, frozen, black, restarts, paused FROM history WHERE at >= ?1 ORDER BY at")?;
    let rows = stmt.query_map(params![now_s - minutes * 60], |r| {
        Ok(HistoryPoint { at: r.get(0)?, kbps: r.get(1)?, channel: r.get(2)?, provider: r.get(3)?, health: r.get(4)?, loss: r.get(5)?, frozen: r.get(6)?, black: r.get(7)?, restarts: r.get(8)?, paused: r.get(9)? })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ZapRecord {
    pub at: i64,
    pub total_ms: i64,
    pub probe_ms: i64,
    pub stop_ms: i64,
    pub launch_ms: i64,
    pub publication_ms: i64,
}

pub fn add_zap(pool: &Pool, zap: &ZapRecord) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT OR REPLACE INTO zaps (at, total_ms, probe_ms, stop_ms, launch_ms, publication_ms) VALUES (?1,?2,?3,?4,?5,?6)",
        params![zap.at, zap.total_ms, zap.probe_ms, zap.stop_ms, zap.launch_ms, zap.publication_ms],
    )?;
    conn.execute("DELETE FROM zaps WHERE at NOT IN (SELECT at FROM zaps ORDER BY at DESC LIMIT ?1)", params![ZAPS_KEPT])?;
    Ok(())
}

pub fn recent_zaps(pool: &Pool) -> Result<Vec<ZapRecord>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT at, total_ms, probe_ms, stop_ms, launch_ms, publication_ms FROM zaps ORDER BY at")?;
    let rows = stmt.query_map([], |r| {
        Ok(ZapRecord { at: r.get(0)?, total_ms: r.get(1)?, probe_ms: r.get(2)?, stop_ms: r.get(3)?, launch_ms: r.get(4)?, publication_ms: r.get(5)? })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn percentile(values: &[i64], p: f64) -> Option<i64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = ((p / 100.0 * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len());
    Some(sorted[rank - 1])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderEvent {
    pub id: i64,
    pub at: i64,
    pub kind: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub channel: Option<String>,
    pub detail: String,
}

const EVENTS_KEPT: i64 = 10;
const MERGE_WINDOW_MS: i64 = 30 * 60 * 1000;

pub fn add_provider_event(pool: &Pool, event: &ProviderEvent) -> Result<()> {
    let conn = pool.get()?;
    let repeated: Option<i64> = conn
        .query_row(
            "SELECT id FROM provider_events WHERE kind = ?1 AND IFNULL(channel,'') = IFNULL(?2,'') AND at >= ?3 ORDER BY id DESC LIMIT 1",
            params![event.kind, event.channel, event.at - MERGE_WINDOW_MS],
            |r| r.get(0),
        )
        .ok();
    match repeated {
        Some(id) => {
            conn.execute("UPDATE provider_events SET at = ?1, detail = ?2 WHERE id = ?3", params![event.at, event.detail, id])?;
        }
        None => {
            conn.execute(
                "INSERT INTO provider_events (at, kind, from_provider, to_provider, channel, detail) VALUES (?1,?2,?3,?4,?5,?6)",
                params![event.at, event.kind, event.from, event.to, event.channel, event.detail],
            )?;
        }
    }
    conn.execute("DELETE FROM provider_events WHERE id NOT IN (SELECT id FROM provider_events ORDER BY id DESC LIMIT ?1)", params![EVENTS_KEPT])?;
    Ok(())
}

pub fn provider_events(pool: &Pool) -> Result<Vec<ProviderEvent>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT id, at, kind, from_provider, to_provider, channel, detail FROM provider_events ORDER BY id DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(ProviderEvent { id: r.get(0)?, at: r.get(1)?, kind: r.get(2)?, from: r.get(3)?, to: r.get(4)?, channel: r.get(5)?, detail: r.get(6)? })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    fn point(at: i64) -> HistoryPoint {
        HistoryPoint { at, kbps: 3000, channel: Some("TF1".into()), provider: Some("primary".into()), health: Some("healthy".into()), ..HistoryPoint::default() }
    }

    #[test]
    fn history_keeps_three_hours_and_answers_a_bounded_window() {
        let pool = test_pool();
        for at in [0, 100, 10_000, 10_900] {
            add_history_point(&pool, &point(at)).unwrap();
        }

        let all = history_since(&pool, 9999, 10_900).unwrap();
        let last_minute = history_since(&pool, 1, 10_900).unwrap();

        assert_eq!(all.iter().map(|p| p.at).collect::<Vec<_>>(), [100, 10_000, 10_900]);
        assert_eq!(last_minute.len(), 1);
    }

    #[test]
    fn a_history_point_carries_names_and_numbers_and_never_a_url() {
        let json = serde_json::to_string(&point(1)).unwrap();

        assert!(!json.contains("http"), "{json}");
    }

    #[test]
    fn zaps_are_percentiles_over_the_fifty_most_recent() {
        let pool = test_pool();
        for at in 0..60 {
            add_zap(&pool, &ZapRecord { at, total_ms: 1000 + at * 10, ..ZapRecord::default() }).unwrap();
        }

        let zaps = recent_zaps(&pool).unwrap();
        let totals: Vec<i64> = zaps.iter().map(|z| z.total_ms).collect();

        assert_eq!(zaps.len(), 50);
        assert_eq!(percentile(&totals, 50.0), Some(1000 + 34 * 10));
        assert_eq!(percentile(&totals, 95.0), Some(1000 + 57 * 10));
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn provider_events_keep_the_last_ten_and_fold_a_repeat_within_thirty_minutes() {
        let pool = test_pool();
        let event = |at, kind: &str, channel: &str| ProviderEvent { at, kind: kind.into(), channel: Some(channel.into()), ..ProviderEvent::default() };
        add_provider_event(&pool, &event(1_000, "failover", "TF1")).unwrap();
        add_provider_event(&pool, &event(2_000, "failover", "TF1")).unwrap();
        for n in 0..12 {
            add_provider_event(&pool, &event(10_000_000 + n, "failback", &format!("c{n}"))).unwrap();
        }

        let kept = provider_events(&pool).unwrap();

        assert_eq!(kept.len(), 10);
        assert!(kept.iter().all(|e| e.kind == "failback"));
        add_provider_event(&pool, &event(1_000, "x", "a")).unwrap();
        add_provider_event(&pool, &event(5_000, "x", "a")).unwrap();
        assert_eq!(provider_events(&pool).unwrap().iter().filter(|e| e.kind == "x").count(), 1);
    }
}
