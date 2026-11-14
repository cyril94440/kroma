use serde::{Deserialize, Serialize};

/// What the core asks for: one title, or one episode of a show, for one user.
/// `exclude` names releases the core already rejected after the file was read.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Ask {
    pub kind: String,
    pub tmdb_id: u64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub user_id: String,
    pub exclude: Vec<String>,
    pub budget_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    NoEngine,
    NoIndexer,
    NoRelease,
}

/// The release that was placed, as the core needs it to read the file back
/// against what it was promised to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    pub key: String,
    pub title: String,
    pub claimed_height: Option<u32>,
}

/// A placed release awaiting the core's verdict: `paths` are the files this
/// call wrote and no others, and nothing is announced until `watch-now-confirm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Imported {
    pub request_id: String,
    pub download_id: String,
    pub release: Release,
    pub paths: Vec<String>,
    pub primary_path: String,
    pub episodes: u32,
    pub multi_part: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Reply {
    Imported(Imported),
    Requested {
        request_id: String,
    },
    Present {
        request_id: Option<String>,
    },
    Unavailable {
        reason: Why,
        request_id: Option<String>,
    },
}

/// The core read the file back and found it not to be what its release
/// promised. Only the download is named: what is undone is what this module
/// recorded for it, never anything the caller lists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Rejection {
    pub download_id: String,
    pub reason: String,
}

/// A release whose other files, read later, were not what it promised: it is
/// blocklisted for the title, and nothing in the library is touched.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Flag {
    pub kind: String,
    pub tmdb_id: u64,
    pub release_title: String,
    pub reason: String,
}

/// The core read the file back and it is what its release promised.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Confirmation {
    pub download_id: String,
}
