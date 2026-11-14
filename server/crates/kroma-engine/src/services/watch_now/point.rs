//! The `acquisition` point's watch-now methods, as the core speaks them. The
//! module that answers declares its own copy of this JSON.

use kroma_module_host::{call_point, Resolver};
use serde::{Deserialize, Serialize};

pub const ACQUISITION: &str = "acquisition";

#[derive(Debug, Serialize)]
pub struct Ask<'a> {
    pub kind: &'a str,
    pub tmdb_id: u64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub user_id: &'a str,
    pub exclude: &'a [String],
    pub budget_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub claimed_height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Imported {
    pub request_id: String,
    pub download_id: String,
    pub release: Release,
    pub paths: Vec<String>,
    pub primary_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Why {
    NoEngine,
    NoIndexer,
    NoRelease,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Reply {
    Imported(Imported),
    Requested {
        request_id: String,
    },
    Unavailable {
        reason: Why,
        #[serde(default)]
        request_id: Option<String>,
    },
}

#[derive(Debug, Serialize)]
pub struct Rejection<'a> {
    pub download_id: &'a str,
    pub release_title: &'a str,
    pub kind: &'a str,
    pub tmdb_id: u64,
    pub reason: &'a str,
    pub paths: &'a [String],
    pub request_id: &'a str,
    pub season: Option<u32>,
}

pub fn ask(resolve: &Resolver, ask: &Ask) -> anyhow::Result<Reply> {
    call_point(resolve, ACQUISITION, "watch-now", ask)
}

pub fn reject(resolve: &Resolver, rejection: &Rejection) -> anyhow::Result<bool> {
    call_point(resolve, ACQUISITION, "watch-now-reject", rejection)
}

pub fn capable(resolve: &Resolver) -> bool {
    call_point(
        resolve,
        ACQUISITION,
        "watch-now-capable",
        &serde_json::json!({}),
    )
    .unwrap_or(false)
}

#[cfg(test)]
mod tests;
