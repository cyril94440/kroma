//! One poll of one magnet: AllDebrid's own phase first, then the transfer onto
//! this server. Progress is a single monotonic number across both, with the
//! remote phase on the first half.

use anyhow::Result;

use crate::api::Magnet;
use crate::types::{TorrentState, TorrentStatus};
use crate::AllDebrid;

const REMOTE_CEILING: f64 = 0.45;

impl AllDebrid {
    pub fn status(&self, client_ref: &str) -> Result<Option<TorrentStatus>> {
        crate::ensure_magnet_id(client_ref)?;
        let Some(magnet) = self.api.magnet(client_ref)? else {
            return Ok(None);
        };
        self.remember(Some(client_ref));
        let reported = match magnet.status_code {
            0..=3 => remote(client_ref, &magnet),
            4 => self.ready_status(client_ref, &magnet)?,
            _ => failed_remote(client_ref, &magnet),
        };
        Ok(Some(reported))
    }
}

pub(crate) fn base(client_ref: &str, magnet: &Magnet) -> TorrentStatus {
    TorrentStatus {
        client_ref: client_ref.to_string(),
        name: magnet.filename.clone(),
        info_hash: None,
        progress: 0.0,
        state: TorrentState::Queued,
        down_bps: 0,
        up_bps: 0,
        peers: 0,
        peers_seen: 0,
        size_bytes: magnet.size,
        save_path: None,
        files: Vec::new(),
        error: None,
        lifetime_downloaded_bytes: 0,
        lifetime_uploaded_bytes: 0,
    }
}

fn remote(client_ref: &str, magnet: &Magnet) -> TorrentStatus {
    let mut status = base(client_ref, magnet);
    status.peers = magnet.seeders;
    status.peers_seen = magnet.seeders;
    status.lifetime_downloaded_bytes = magnet.downloaded;
    match magnet.status_code {
        0 => {}
        1 => {
            status.state = TorrentState::Downloading;
            status.down_bps = magnet.download_speed;
            status.progress = REMOTE_CEILING * fraction(magnet);
        }
        _ => {
            status.state = TorrentState::Downloading;
            status.progress = REMOTE_CEILING;
        }
    }
    status
}

fn failed_remote(client_ref: &str, magnet: &Magnet) -> TorrentStatus {
    let mut status = base(client_ref, magnet);
    status.state = TorrentState::Error;
    status.progress = REMOTE_CEILING * fraction(magnet);
    status.error = Some(format!(
        "AllDebrid: {} (code {})",
        failure_text(magnet),
        magnet.status_code
    ));
    status
}

fn fraction(magnet: &Magnet) -> f64 {
    if magnet.size == 0 {
        return 0.0;
    }
    (magnet.downloaded as f64 / magnet.size as f64).clamp(0.0, 1.0)
}

fn failure_text(magnet: &Magnet) -> &str {
    if !magnet.status.is_empty() {
        return &magnet.status;
    }
    match magnet.status_code {
        5 => "upload failed",
        6 => "internal error on unpacking",
        7 => "not downloaded in 20 minutes",
        8 => "file too big",
        10 => "download took more than 72 hours",
        11 => "deleted on the hoster website",
        12 | 13 => "processing failed",
        14 => "error contacting the tracker",
        15 => "file not available, no peer",
        _ => "internal error",
    }
}
