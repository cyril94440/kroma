use anyhow::Result;
use rusqlite::params;

use super::Pool;
use crate::model::{Channel, ProviderId};

const BATCH: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogueState {
    pub fetched_at: i64,
    pub channels: i64,
    pub error: Option<String>,
}

pub fn replace_channels(pool: &Pool, provider: ProviderId, channels: &[Channel], now_ms: i64) -> Result<()> {
    let mut conn = pool.get()?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM channels WHERE provider = ?1", params![provider.as_str()])?;
    for chunk in channels.chunks(BATCH) {
        let rows = vec!["(?,?,?,?,?,?)"; chunk.len()].join(",");
        let mut values: Vec<rusqlite::types::Value> = Vec::with_capacity(chunk.len() * 6);
        for channel in chunk {
            values.push(provider.as_str().to_string().into());
            values.push(channel.stream_id.into());
            values.push(channel.name.clone().into());
            values.push(channel.epg_id.clone().map_or(rusqlite::types::Value::Null, Into::into));
            values.push(channel.group.clone().into());
            values.push(channel.url.clone().map_or(rusqlite::types::Value::Null, Into::into));
        }
        tx.execute(
            &format!("INSERT OR REPLACE INTO channels (provider, stream_id, name, epg_id, grp, url) VALUES {rows}"),
            rusqlite::params_from_iter(values),
        )?;
    }
    tx.execute(
        "INSERT INTO catalogue_state (provider, fetched_at, channels, error) VALUES (?1, ?2, ?3, NULL) \
         ON CONFLICT(provider) DO UPDATE SET fetched_at = excluded.fetched_at, channels = excluded.channels, error = NULL",
        params![provider.as_str(), now_ms, channels.len() as i64],
    )?;
    tx.commit()?;
    Ok(())
}

pub fn load_channels(pool: &Pool, provider: ProviderId) -> Result<Vec<Channel>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT stream_id, name, epg_id, grp, url FROM channels WHERE provider = ?1 ORDER BY stream_id")?;
    let rows = stmt.query_map(params![provider.as_str()], |r| {
        let channel = Channel::new(provider, r.get(0)?, &r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?.as_deref(), &r.get::<_, String>(3)?);
        Ok(match r.get::<_, Option<String>>(4)? {
            Some(url) => channel.with_url(&url),
            None => channel,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn note_catalogue_failure(pool: &Pool, provider: ProviderId, error: &str, now_ms: i64) -> Result<()> {
    pool.get()?.execute(
        "INSERT INTO catalogue_state (provider, fetched_at, channels, error) VALUES (?1, 0, 0, ?2) \
         ON CONFLICT(provider) DO UPDATE SET error = excluded.error",
        params![provider.as_str(), error],
    )?;
    let _ = now_ms;
    Ok(())
}

pub fn catalogue_state(pool: &Pool, provider: ProviderId) -> Result<CatalogueState> {
    let conn = pool.get()?;
    let found = conn.query_row(
        "SELECT fetched_at, channels, error FROM catalogue_state WHERE provider = ?1",
        params![provider.as_str()],
        |r| Ok(CatalogueState { fetched_at: r.get(0)?, channels: r.get(1)?, error: r.get(2)? }),
    );
    match found {
        Ok(state) => Ok(state),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CatalogueState::default()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[test]
    fn a_catalogue_is_replaced_whole_and_read_back_with_its_urls() {
        let pool = test_pool();
        let first = vec![Channel::new(ProviderId::Primary, 1, "TF1", Some("tf1.fr"), "News"), Channel::new(ProviderId::Primary, 2, "M6", None, "").with_url("http://h/m6.ts")];
        replace_channels(&pool, ProviderId::Primary, &first, 10).unwrap();
        replace_channels(&pool, ProviderId::Backup, &[Channel::new(ProviderId::Backup, 1, "TF1 FHD", None, "")], 10).unwrap();

        replace_channels(&pool, ProviderId::Primary, &first[..1], 20).unwrap();

        let loaded = load_channels(&pool, ProviderId::Primary).unwrap();
        assert_eq!((loaded.len(), loaded[0].epg_id.as_deref(), loaded[0].group.as_str()), (1, Some("tf1.fr"), "News"));
        assert_eq!(load_channels(&pool, ProviderId::Backup).unwrap().len(), 1);
        assert_eq!(catalogue_state(&pool, ProviderId::Primary).unwrap(), CatalogueState { fetched_at: 20, channels: 1, error: None });
    }

    #[test]
    fn a_failure_is_remembered_without_dropping_the_channels_already_held() {
        let pool = test_pool();
        replace_channels(&pool, ProviderId::Primary, &[Channel::new(ProviderId::Primary, 1, "TF1", None, "")], 10).unwrap();

        note_catalogue_failure(&pool, ProviderId::Primary, "HTTP 503", 11).unwrap();

        assert_eq!(load_channels(&pool, ProviderId::Primary).unwrap().len(), 1);
        assert_eq!(catalogue_state(&pool, ProviderId::Primary).unwrap().error.as_deref(), Some("HTTP 503"));
    }

    #[test]
    fn a_large_catalogue_goes_in_by_batches() {
        let pool = test_pool();
        let many: Vec<Channel> = (0..450).map(|i| Channel::new(ProviderId::Primary, i, &format!("Chan {i}"), None, "")).collect();

        replace_channels(&pool, ProviderId::Primary, &many, 1).unwrap();

        assert_eq!(load_channels(&pool, ProviderId::Primary).unwrap().len(), 450);
    }
}
