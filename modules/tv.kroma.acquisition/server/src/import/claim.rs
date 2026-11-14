use kroma_module_sdk::db::Pool;
use kroma_module_sdk::primitives::now_ms;

const STALE_MS: i64 = 10 * 60 * 1000;

pub(super) struct Claim<'a> {
    store: &'a Pool,
    download_id: String,
}

impl<'a> Claim<'a> {
    /// The right to import one download, held until this value is dropped. A
    /// claim older than ten minutes belongs to an import that died, and is taken
    /// over. `None` when another import holds it.
    pub(super) fn take(store: &'a Pool, download_id: &str) -> Option<Self> {
        let now = now_ms();
        let conn = store.get().ok()?;
        let changed = conn
            .execute(
                "INSERT INTO import_claims (download_id, claimed_at) VALUES (?1, ?2) \
                 ON CONFLICT(download_id) DO UPDATE SET claimed_at = ?2 WHERE claimed_at < ?3",
                rusqlite::params![download_id, now, now - STALE_MS],
            )
            .ok()?;
        (changed == 1).then(|| Self {
            store,
            download_id: download_id.to_string(),
        })
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        if let Ok(conn) = self.store.get() {
            let _ = conn.execute(
                "DELETE FROM import_claims WHERE download_id = ?1",
                [&self.download_id],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (kroma_module_sdk::testing::TempDir, Pool) {
        let dir = kroma_module_sdk::testing::temp_dir("import-claim");
        let pool = kroma_module_sdk::db::open(&dir.path().join("module.sqlite")).unwrap();
        crate::rules::schema::install(&pool);
        (dir, pool)
    }

    #[test]
    fn a_download_is_imported_by_one_caller_at_a_time() {
        let (_dir, pool) = store();

        let first = Claim::take(&pool, "d1");
        let second = Claim::take(&pool, "d1");

        assert!(first.is_some());
        assert!(second.is_none());
        assert!(Claim::take(&pool, "d2").is_some());
    }

    #[test]
    fn a_released_claim_can_be_taken_again() {
        let (_dir, pool) = store();

        drop(Claim::take(&pool, "d1"));

        assert!(Claim::take(&pool, "d1").is_some());
    }

    #[test]
    fn a_claim_that_outlived_its_importer_is_taken_over() {
        let (_dir, pool) = store();
        let held = Claim::take(&pool, "d1").unwrap();
        pool.get()
            .unwrap()
            .execute("UPDATE import_claims SET claimed_at = claimed_at - ?1", [STALE_MS + 1])
            .unwrap();
        std::mem::forget(held);

        assert!(Claim::take(&pool, "d1").is_some());
    }
}
