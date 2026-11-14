use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::Pool;

pub fn kv_get(pool: &Pool, key: &str) -> Result<Option<String>> {
    let conn = pool.get()?;
    Ok(conn
        .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| r.get(0))
        .optional()?)
}

pub fn kv_set(pool: &Pool, key: &str, value: &str) -> Result<()> {
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
pub fn kv_delete(pool: &Pool, key: &str) -> Result<()> {
    pool.get()?.execute("DELETE FROM kv WHERE key = ?1", params![key])?;
    Ok(())
}

pub fn kv_get_json<T: DeserializeOwned>(pool: &Pool, key: &str) -> Option<T> {
    kv_get(pool, key).ok().flatten().and_then(|text| serde_json::from_str(&text).ok())
}

pub fn kv_set_json<T: Serialize>(pool: &Pool, key: &str, value: &T) -> Result<()> {
    kv_set(pool, key, &serde_json::to_string(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[test]
    fn a_value_is_replaced_read_back_and_deleted() {
        let pool = test_pool();

        assert_eq!(kv_get(&pool, "a").unwrap(), None);
        kv_set(&pool, "a", "1").unwrap();
        kv_set(&pool, "a", "2").unwrap();
        assert_eq!(kv_get(&pool, "a").unwrap().as_deref(), Some("2"));
        kv_delete(&pool, "a").unwrap();
        assert_eq!(kv_get(&pool, "a").unwrap(), None);
    }

    #[test]
    fn json_that_no_longer_parses_reads_as_absent() {
        let pool = test_pool();
        kv_set(&pool, "j", "{not json").unwrap();

        assert_eq!(kv_get_json::<Vec<u8>>(&pool, "j"), None);
        kv_set_json(&pool, "j", &vec![1u8, 2]).unwrap();
        assert_eq!(kv_get_json::<Vec<u8>>(&pool, "j"), Some(vec![1, 2]));
    }
}
