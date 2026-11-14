//! The types this module works in, and reports on the `download-client` point.
//!
//! Its own, not a shared crate's: what it agrees on with the download module is
//! the JSON in [`crate::port`], which each side's tests pin.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientDef {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentState {
    Queued,
    Downloading,
    Completed,
    Error,
}

#[derive(Debug, Clone)]
pub struct TorrentStatus {
    pub client_ref: String,
    pub name: String,
    pub info_hash: Option<String>,
    pub progress: f64,
    pub state: TorrentState,
    pub down_bps: u64,
    pub up_bps: u64,
    pub peers: u32,
    pub peers_seen: u32,
    pub size_bytes: u64,
    pub save_path: Option<String>,
    pub files: Vec<String>,
    pub error: Option<String>,
    pub lifetime_downloaded_bytes: u64,
    pub lifetime_uploaded_bytes: u64,
}
