use std::time::Instant;

use anyhow::{anyhow, Result};
use serde_json::json;

use kroma_module_sdk::domain::RequestStatus;
use kroma_module_sdk::host::{Event, HostCtx};
use kroma_module_sdk::primitives::now_ms;

use crate::db::{self, DownloadRow};
use crate::TorrentState;

use super::{DownloadManager, InstantOutcome, InstantStatus};

pub struct Pending {
    pub(super) row: DownloadRow,
    pub(super) key: String,
    pub(super) client: db::DownloadClientRow,
    pub(super) wanted_ids: Vec<String>,
    pub(super) settle_by: Instant,
}

pub enum Begun {
    Done(InstantOutcome),
    Settling(Pending),
}

pub enum Settle {
    Ready,
    Waiting(Option<String>),
    Failed { reason: String, deterministic: bool },
}

impl DownloadManager {
    /// One look at the engine's answer for a row that is settling.
    pub fn settle_step(&self, host: &dyn HostCtx, pending: &Pending) -> Settle {
        let engine = match self.engine_for(&pending.client) {
            Ok(engine) => engine,
            Err(error) => return Settle::Waiting(Some(format!("{error:#}"))),
        };
        match engine.status(&pending.row.client_ref) {
            Ok(Some(status)) if status.state == TorrentState::Error => Settle::Failed {
                reason: status
                    .error
                    .unwrap_or_else(|| "the engine failed the release".into()),
                deterministic: true,
            },
            Ok(Some(status)) => {
                if self.record_status(host, &pending.row, &status, false) {
                    Settle::Ready
                } else {
                    Settle::Waiting(None)
                }
            }
            Ok(None) => Settle::Failed {
                reason: "the engine forgot the release it had just kept".into(),
                deterministic: true,
            },
            Err(error) => Settle::Waiting(Some(format!("{error:#}"))),
        }
    }

    /// The second half: a ready row is the grab, a failed one is released and
    /// reported so the caller can go on to the next release.
    pub fn instant_finish(
        &self,
        host: &dyn HostCtx,
        pending: Pending,
        step: Settle,
    ) -> Result<InstantOutcome> {
        match step {
            Settle::Ready => {
                let conn = self.core().get()?;
                let download = db::get_download(&conn, &pending.row.id)?
                    .ok_or_else(|| anyhow!("the download vanished"))?;
                Ok(InstantOutcome {
                    status: InstantStatus::Grabbed,
                    key: Some(pending.key),
                    download: Some(download),
                    reason: None,
                    deterministic: false,
                })
            }
            Settle::Failed {
                reason,
                deterministic,
            } => Ok(self.release_failed(host, pending, reason, deterministic)),
            Settle::Waiting(last) => {
                let Settle::Failed {
                    reason,
                    deterministic,
                } = Settle::timed_out(last)
                else {
                    unreachable!()
                };
                Ok(self.release_failed(host, pending, reason, deterministic))
            }
        }
    }

    pub(super) fn release_failed(
        &self,
        host: &dyn HostCtx,
        pending: Pending,
        reason: String,
        deterministic: bool,
    ) -> InstantOutcome {
        let _ = db::set_download_status(self.core(), &pending.row.id, "removed", Some(&reason));
        let _ = db::set_wanted_status(self.core(), &pending.wanted_ids, "wanted", now_ms());
        if let Ok(engine) = self.engine_for(&pending.client) {
            let _ = engine.discard(&pending.row.client_ref);
        }
        if let Some(request_id) = &pending.row.request_id {
            host.publish(Event::new(
                "request.updated",
                json!({ "id": request_id, "status": RequestStatus::Approved.as_str() }),
            ));
        }
        InstantOutcome {
            status: InstantStatus::Failed,
            key: Some(pending.key),
            download: None,
            reason: Some(reason),
            deterministic,
        }
    }
}

impl Pending {
    pub fn expired(&self) -> bool {
        Instant::now() >= self.settle_by
    }
}

impl Settle {
    pub(super) fn timed_out(last: Option<String>) -> Self {
        Settle::Failed {
            reason: last.unwrap_or_else(|| "the engine did not finish preparing the release in time".into()),
            deterministic: false,
        }
    }
}

