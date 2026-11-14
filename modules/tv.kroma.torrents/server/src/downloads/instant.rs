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
use crate::{InstantAnswer, InstantCandidate, InstantUnsupported};

use super::{DownloadManager, GrabSpec};

pub use settle::{Begun, Pending, Settle};

pub const MAX_INSTANT_CANDIDATES: usize = 8;
pub(super) const SETTLE_FOR: Duration = Duration::from_secs(8);
const PROBE_BUDGET: Duration = Duration::from_secs(3);
const SETTLE_EVERY: Duration = Duration::from_millis(250);

/// One release offered to an instant grab, with the spec its row is made from.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstantRelease {
    pub key: String,
    pub spec: GrabSpec,
}

/// What an instant grab came to. `failed` is a release the engine held that
/// could not be settled into a completed row: the caller skips its `key`, and
/// blocklists it when `deterministic` says the release itself is at fault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstantStatus {
    Grabbed,
    Nothing,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantOutcome {
    pub status: InstantStatus,
    pub key: Option<String>,
    pub download: Option<DownloadRow>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub deterministic: bool,
}

impl InstantOutcome {
    fn of(status: InstantStatus) -> Self {
        Self {
            status,
            key: None,
            download: None,
            reason: None,
            deterministic: false,
        }
    }
}

impl DownloadManager {
    /// Whether the preferred enabled client can say which releases it holds already.
    pub fn instant_capable(&self) -> bool {
        let Ok(client) = self.preferred_client() else {
            return false;
        };
        if client.kind == crate::engine::EMBEDDED_KIND {
            return false;
        }
        self.engine_for(&client)
            .and_then(|engine| engine.instant(&[], PROBE_BUDGET))
            .is_ok()
    }

    /// Asks the preferred client which of the ranked releases it holds already
    /// and, for the best one, records the download as completed, all within
    /// `budget`. The kill switch applies unless the engine states that it moves
    /// no peer traffic through this machine. The parallelism cap does not,
    /// because the row never waits for a slot.
    pub fn grab_instant(
        &self,
        host: &dyn HostCtx,
        releases: Vec<InstantRelease>,
        budget: Duration,
    ) -> Result<InstantOutcome> {
        let pending = match self.instant_begin(host, releases, budget)? {
            Begun::Done(outcome) => return Ok(outcome),
            Begun::Settling(pending) => pending,
        };
        loop {
            let step = self.settle_step(host, &pending);
            let step = match step {
                Settle::Waiting(last) if pending.expired() => Settle::timed_out(last),
                Settle::Waiting(_) => {
                    std::thread::sleep(SETTLE_EVERY);
                    continue;
                }
                done => done,
            };
            return self.instant_finish(host, pending, step);
        }
    }

    /// The first half of [`DownloadManager::grab_instant`]: what the engine
    /// holds, and the row recorded for it, still settling.
    pub fn instant_begin(
        &self,
        host: &dyn HostCtx,
        releases: Vec<InstantRelease>,
        budget: Duration,
    ) -> Result<Begun> {
        let deadline = Instant::now() + budget;
        let client = self.preferred_client()?;
        if client.kind == crate::engine::EMBEDDED_KIND {
            return Ok(Begun::Done(InstantOutcome::of(InstantStatus::Unsupported)));
        }
        let engine = self.engine_for(&client)?;
        let offered = self.not_already_queued(releases)?;
        let probe = match engine.instant(&[], PROBE_BUDGET) {
            Ok(answer) => answer,
            Err(error) if error.is::<InstantUnsupported>() => {
                return Ok(Begun::Done(InstantOutcome::of(InstantStatus::Unsupported)));
            }
            Err(error) => return Err(error),
        };
        if !probe.peerless && !self.gate_open() {
            return Ok(Begun::Done(InstantOutcome::of(InstantStatus::Nothing)));
        }
        let candidates: Vec<InstantCandidate> = offered
            .iter()
            .map(|release| InstantCandidate {
                key: release.key.clone(),
                magnet_or_url: release.spec.magnet_or_url.trim().to_string(),
            })
            .collect();
        let hit = match engine.instant(&candidates, budget) {
            Ok(InstantAnswer { hit: Some(hit), .. }) => hit,
            Ok(_) => return Ok(Begun::Done(InstantOutcome::of(InstantStatus::Nothing))),
            Err(error) if error.is::<InstantUnsupported>() => {
                return Ok(Begun::Done(InstantOutcome::of(InstantStatus::Unsupported)));
            }
            Err(error) => return Err(error),
        };
        let chosen = offered
            .into_iter()
            .find(|release| release.key == hit.key)
            .ok_or_else(|| anyhow!("the engine kept a release that was not offered"))?;
        let wanted_ids = chosen.spec.wanted_ids.clone();
        let row = self.record_instant(host, &client, chosen.spec, &hit.client_ref)?;
        Ok(Begun::Settling(Pending {
            row,
            key: hit.key,
            client,
            wanted_ids,
            settle_by: deadline.min(Instant::now() + SETTLE_FOR),
        }))
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
        Ok(row)
    }
}

mod settle;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_settle;
