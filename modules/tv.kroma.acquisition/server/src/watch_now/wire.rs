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

/// The release that was imported, as the core needs it to read the file back
/// against what it was promised to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    pub key: String,
    pub title: String,
    pub claimed_height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Imported {
    pub request_id: String,
    pub download_id: String,
    pub release: Release,
    pub paths: Vec<String>,
    pub primary_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Reply {
    Imported(Imported),
    Requested {
        request_id: String,
    },
    Unavailable {
        reason: Why,
        request_id: Option<String>,
    },
}

/// A file the core read back and found not to be what its release promised.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Rejection {
    pub download_id: String,
    pub release_title: String,
    pub kind: String,
    pub tmdb_id: u64,
    pub reason: String,
    pub paths: Vec<String>,
    pub request_id: Option<String>,
    pub season: Option<u32>,
}
