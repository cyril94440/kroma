use std::time::{Duration, Instant};

use crate::db;

use super::DownloadManager;

pub(in crate::downloads) const GIVE_UP_AFTER: Duration = Duration::from_secs(15 * 60);

impl DownloadManager {
    pub(super) fn poll_answered(&self, row_id: &str) {
        self.unanswered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(row_id);
    }

    pub(super) fn poll_failed(&self, row: &db::DownloadRow, error: &anyhow::Error) {
        self.poll_failed_at(row, error, Instant::now(), GIVE_UP_AFTER);
    }

    pub(in crate::downloads) fn poll_failed_at(
        &self,
        row: &db::DownloadRow,
        error: &anyhow::Error,
        now: Instant,
        give_up_after: Duration,
    ) {
        let first = *self
            .unanswered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entry(row.id.clone())
            .or_insert(now);
        if now.saturating_duration_since(first) < give_up_after {
            return;
        }
        self.poll_answered(&row.id);
        let reason = format!(
            "the download client has not answered for {} minutes: {error:#}",
            give_up_after.as_secs() / 60
        );
        let _ = db::set_download_status(self.core(), &row.id, "failed", Some(&reason));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::downloads::test_support::{added, manager_over};

    fn status_of(manager: &DownloadManager, id: &str) -> (String, Option<String>) {
        let row = db::get_download(&manager.core().get().unwrap(), id)
            .unwrap()
            .unwrap();
        (row.status, row.error)
    }

    #[test]
    fn a_client_that_keeps_failing_stops_looking_like_an_eternal_download() {
        let (_host, manager) = manager_over("torrents-unreachable", &[added("d1")]);
        let row = added("d1");
        let start = Instant::now();
        let error = anyhow::anyhow!("HTTP 502 revoked");

        manager.poll_failed_at(&row, &error, start, GIVE_UP_AFTER);
        manager.poll_failed_at(&row, &error, start + GIVE_UP_AFTER / 2, GIVE_UP_AFTER);
        assert_eq!(status_of(&manager, "d1").0, "downloading");
        manager.poll_failed_at(&row, &error, start + GIVE_UP_AFTER, GIVE_UP_AFTER);

        let (status, reason) = status_of(&manager, "d1");
        assert_eq!(status, "failed");
        assert!(reason.unwrap().contains("revoked"));
    }

    #[test]
    fn one_answer_in_between_restarts_the_clock() {
        let (_host, manager) = manager_over("torrents-unreachable-reset", &[added("d1")]);
        let row = added("d1");
        let start = Instant::now();
        let error = anyhow::anyhow!("down");

        manager.poll_failed_at(&row, &error, start, GIVE_UP_AFTER);
        manager.poll_answered("d1");
        manager.poll_failed_at(&row, &error, start + GIVE_UP_AFTER, GIVE_UP_AFTER);

        assert_eq!(status_of(&manager, "d1").0, "downloading");
    }
}
