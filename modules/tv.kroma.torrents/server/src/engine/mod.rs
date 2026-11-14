//! What this module needs of a torrent engine, and how it reaches one.

pub mod remote;

pub const EMBEDDED_KIND: &str = "rqbit";

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AddTorrentReq<'a> {
    pub magnet_or_url: &'a str,
    pub download_dir: Option<&'a str>,
    pub label: &'a str,
    pub only_files: Option<&'a [usize]>,
    pub torrent_bytes: Option<&'a [u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentState {
    Queued,
    Downloading,
    Seeding,
    Paused,
    Completed,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    // An engine module ships on its own tag, so a peer still on the pre-rename
    // keys, or on one older than the fields, reaches this module every day.
    #[serde(default, alias = "downloaded_bytes")]
    pub lifetime_downloaded_bytes: u64,
    #[serde(default, alias = "uploaded_bytes")]
    pub lifetime_uploaded_bytes: u64,
}

impl TorrentStatus {
    /// Running with no live peer, whatever it already holds. A torrent still
    /// checking its files on disk is queued, not running.
    pub fn is_starving(&self) -> bool {
        self.state == TorrentState::Downloading && self.peers == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFileEntry {
    pub index: usize,
    pub path: String,
    pub size_bytes: u64,
}

/// One release offered to [`DownloadClient::instant`], under the key its
/// caller finds it by again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstantCandidate {
    pub key: String,
    pub magnet_or_url: String,
}

/// The candidate an engine already holds, and the reference it answers to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstantHit {
    pub key: String,
    pub client_ref: String,
}

/// What an engine answered to an instant ask: the candidate it kept, and its
/// statement that it moves no peer traffic through this machine. Without that
/// statement the downloads module treats the engine as a peer and holds it to
/// the VPN kill switch.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstantAnswer {
    pub hit: Option<InstantHit>,
    pub peerless: bool,
}

impl InstantAnswer {
    /// Reads either shape an engine may answer: `{ "hit", "peerless" }`, or the
    /// bare hit (or `null`) of an engine that predates the statement, which
    /// never counts as peerless.
    pub fn from_json(value: serde_json::Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        struct Stated {
            #[serde(default)]
            hit: Option<InstantHit>,
            #[serde(default)]
            peerless: bool,
        }
        match &value {
            serde_json::Value::Null => Ok(Self::default()),
            serde_json::Value::Object(map) if map.contains_key("hit") || map.contains_key("peerless") => {
                let stated: Stated = serde_json::from_value(value)?;
                Ok(Self {
                    hit: stated.hit,
                    peerless: stated.peerless,
                })
            }
            _ => Ok(Self {
                hit: Some(serde_json::from_value(value)?),
                peerless: false,
            }),
        }
    }
}

/// An engine that cannot say which releases it holds already.
#[derive(Debug)]
pub struct InstantUnsupported;

impl std::fmt::Display for InstantUnsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("this download client cannot tell which releases are ready at once")
    }
}

impl std::error::Error for InstantUnsupported {}

pub trait DownloadClient: Send + Sync {
    fn kind(&self) -> &'static str;
    // Reachability probe; returns a human-readable version string.
    fn test(&self) -> anyhow::Result<String>;
    // Returns the engine's identifier for the new torrent (`client_ref`).
    fn add(&self, req: &AddTorrentReq) -> anyhow::Result<String>;
    // Metadata-only, no download. `torrent_bytes` are the pre-fetched
    // `.torrent` file; when `None` the engine resolves `magnet_or_url` itself.
    // Not every engine can do this (external clients don't expose a list-only
    // add) - the default reports it as unsupported.
    fn list_files(
        &self,
        _magnet_or_url: &str,
        _torrent_bytes: Option<&[u8]>,
    ) -> anyhow::Result<Vec<TorrentFileEntry>> {
        anyhow::bail!("this download client cannot list a torrent's files before adding it")
    }
    // The first of the ranked candidates the engine can hand over without
    // fetching anything, left added; every other candidate is left out, all
    // within `budget`. An empty list asks only whether the engine can do this,
    // and what it says about peer traffic.
    fn instant(
        &self,
        _candidates: &[InstantCandidate],
        _budget: std::time::Duration,
    ) -> anyhow::Result<InstantAnswer> {
        Err(InstantUnsupported.into())
    }
    // The release was turned down: remove it and, where the engine can tell it
    // made it, whatever it kept on its side.
    fn discard(&self, client_ref: &str) -> anyhow::Result<()> {
        self.remove(client_ref, true)
    }
    // `Ok(None)` = the engine no longer knows this torrent.
    fn status(&self, client_ref: &str) -> anyhow::Result<Option<TorrentStatus>>;
    fn pause(&self, client_ref: &str) -> anyhow::Result<()>;
    fn resume(&self, client_ref: &str) -> anyhow::Result<()>;
    // Force a tracker / DHT re-announce now. Best-effort; the default is a
    // no-op.
    fn reannounce(&self, _client_ref: &str) -> anyhow::Result<()> {
        Ok(())
    }
    fn remove(&self, client_ref: &str, delete_data: bool) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientDef {
    pub kind: String,
    pub url: String,
    pub username: String,
    pub password: String,
}

pub fn magnet_info_hash(uri: &str) -> Option<String> {
    let lower = uri.to_ascii_lowercase();
    let idx = lower.find("xt=urn:btih:")?;
    let hash: String = lower[idx + "xt=urn:btih:".len()..]
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    // 40-char hex (v1) or 32-char base32.
    (hash.len() == 40 || hash.len() == 32).then_some(hash)
}

#[cfg(test)]
mod tests;
