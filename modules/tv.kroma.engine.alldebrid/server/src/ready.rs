use anyhow::Result;

use crate::api::Magnet;
use crate::status::base;
use crate::types::{TorrentState, TorrentStatus};
use crate::AllDebrid;

impl AllDebrid {
    /// What `status` reports for a magnet AllDebrid has finished: the one place
    /// "ready" becomes `completed`, `save_path` and `files`.
    pub fn ready_status(&self, client_ref: &str, magnet: &Magnet) -> Result<TorrentStatus> {
        let mut status = base(client_ref, magnet);
        status.state = TorrentState::Completed;
        status.progress = 1.0;
        Ok(status)
    }
}
