//! Grabbing a release the engine already holds: the row is born tied to the
//! engine's reference and completed in the same call, with no second add.

use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use kroma_module_sdk::domain::RequestStatus;
use kroma_module_sdk::host::{Event, HostCtx};
use kroma_module_sdk::primitives::now_ms;

use crate::db::{self, DownloadRow};
use crate::{InstantCandidate, InstantUnsupported, TorrentState};

use super::{DownloadManager, GrabSpec};

pub const MAX_INSTANT_CANDIDATES: usize = 8;
const SETTLE_FOR: Duration = Duration::from_secs(8);
const SETTLE_EVERY: Duration = Duration::from_millis(250);

/// One release offered to an instant grab, with the spec its row is made from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstantRelease {
    pub key: String,
    pub spec: GrabSpec,
}

/// What an instant grab came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstantStatus {
    Grabbed,
    Nothing,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantOutcome {
    pub status: InstantStatus,
    pub key: Option<String>,
    pub download: Option<DownloadRow>,
}

impl InstantOutcome {
    fn of(status: InstantStatus) -> Self {
        Self {
            status,
            key: None,
            download: None,
        }
    }
}

impl DownloadManager {
    /// Whether the preferred enabled client can say which releases it holds.
    pub fn instant_capable(&self) -> bool {
        let Ok(client) = self.preferred_client() else {
            return false;
        };
        if client.kind == crate::engine::EMBEDDED_KIND {
            return false;
        }
        self.engine_for(&client)
            .and_then(|engine| engine.instant(&[]))
            .is_ok()
    }

    /// Asks the preferred client which of the ranked releases it holds already
    /// and, for the best one, records the download as completed. The kill switch
    /// is not consulted: an engine that implements `instant` moves no data
    /// through this machine, so there is no peer for the switch to hide behind.
    /// The parallelism cap does not apply either, because the row never waits
    /// for a slot.
    pub fn grab_instant(
        &self,
        host: &dyn HostCtx,
        releases: Vec<InstantRelease>,
    ) -> Result<InstantOutcome> {
        let client = self.preferred_client()?;
        if client.kind == crate::engine::EMBEDDED_KIND {
            return Ok(InstantOutcome::of(InstantStatus::Unsupported));
        }
        let engine = self.engine_for(&client)?;
        let offered = self.not_already_queued(releases)?;
        let candidates: Vec<InstantCandidate> = offered
            .iter()
            .map(|release| InstantCandidate {
                key: release.key.clone(),
                magnet_or_url: release.spec.magnet_or_url.trim().to_string(),
            })
            .collect();
        let hit = match engine.instant(&candidates) {
            Ok(Some(hit)) => hit,
            Ok(None) => return Ok(InstantOutcome::of(InstantStatus::Nothing)),
            Err(error) if error.is::<InstantUnsupported>() => {
                return Ok(InstantOutcome::of(InstantStatus::Unsupported));
            }
            Err(error) => return Err(error),
        };
        let chosen = offered
            .into_iter()
            .find(|release| release.key == hit.key)
            .ok_or_else(|| anyhow!("the engine kept a release that was not offered"))?;
        let row = self.record_instant(host, &client, &*engine, chosen.spec, &hit.client_ref)?;
        Ok(InstantOutcome {
            status: InstantStatus::Grabbed,
            key: Some(hit.key),
            download: Some(row),
        })
    }

    fn preferred_client(&self) -> Result<db::DownloadClientRow> {
        let clients = self.store().get()?;
        db::preferred_download_client(&clients)?
            .ok_or_else(|| anyhow!("no enabled download client"))
    }

    fn not_already_queued(&self, releases: Vec<InstantRelease>) -> Result<Vec<InstantRelease>> {
        let conn = self.core().get()?;
        let mut offered = Vec::new();
        for release in releases.into_iter().take(MAX_INSTANT_CANDIDATES) {
            let url = release.spec.magnet_or_url.trim();
            if url.is_empty() || db::active_download_by_url(&conn, url)?.is_some() {
                continue;
            }
            offered.push(release);
        }
        Ok(offered)
    }

    fn record_instant(
        &self,
        host: &dyn HostCtx,
        client: &db::DownloadClientRow,
        engine: &dyn crate::DownloadClient,
        spec: GrabSpec,
        client_ref: &str,
    ) -> Result<DownloadRow> {
        let conn = self.core().get()?;
        if db::other_active_download_with_ref(&conn, "", client_ref)?.is_some() {
            bail!("this release is already downloading");
        }
        drop(conn);
        let wanted_ids = spec.wanted_ids.clone();
        let row = self.new_row(client, spec, "downloading", client_ref);
        db::insert_download(self.core(), &row)?;
        db::set_wanted_status(self.core(), &wanted_ids, "grabbed", now_ms())?;
        if let Some(request_id) = &row.request_id {
            host.publish(Event::new(
                "request.updated",
                json!({ "id": request_id, "status": RequestStatus::Downloading.as_str() }),
            ));
        }
        match self.settle(host, engine, &row) {
            Ok(()) => {}
            Err(error) => {
                let _ = db::set_download_status(
                    self.core(),
                    &row.id,
                    "failed",
                    Some(&format!("{error:#}")),
                );
                let _ = engine.remove(client_ref, true);
                return Err(error);
            }
        }
        let conn = self.core().get()?;
        db::get_download(&conn, &row.id)?.ok_or_else(|| anyhow!("the download vanished"))
    }

    fn settle(
        &self,
        host: &dyn HostCtx,
        engine: &dyn crate::DownloadClient,
        row: &DownloadRow,
    ) -> Result<()> {
        let started = Instant::now();
        loop {
            let status = engine
                .status(&row.client_ref)?
                .ok_or_else(|| anyhow!("the engine forgot the release it had just kept"))?;
            if status.state == TorrentState::Error {
                bail!(
                    "{}",
                    status
                        .error
                        .as_deref()
                        .unwrap_or("the engine failed the release")
                );
            }
            if self.record_status(host, row, &status, false) {
                return Ok(());
            }
            if started.elapsed() >= SETTLE_FOR {
                bail!("the engine did not finish preparing the release in time");
            }
            std::thread::sleep(SETTLE_EVERY);
        }
    }
}

#[cfg(test)]
mod tests;
