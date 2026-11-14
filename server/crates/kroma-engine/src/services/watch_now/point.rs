//! The `acquisition` point's watch-now methods, as the core speaks them. The
//! module that answers declares its own copy of this JSON.

use std::time::Duration;

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

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct Imported {
    pub request_id: String,
    pub download_id: String,
    pub release: Release,
    pub paths: Vec<String>,
    pub primary_path: String,
    #[serde(default = "one")]
    pub episodes: u32,
    #[serde(default)]
    pub multi_part: bool,
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
    Present {
        #[serde(default)]
        request_id: Option<String>,
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
    pub reason: &'a str,
}

#[derive(Debug, Serialize)]
pub struct Confirmation<'a> {
    pub download_id: &'a str,
}

#[derive(Debug, Serialize)]
pub struct Flag<'a> {
    pub kind: &'a str,
    pub tmdb_id: u64,
    pub release_title: &'a str,
    pub reason: &'a str,
}

const SLACK: Duration = Duration::from_secs(5);

/// Asks within `ask.budget_ms`: the call itself is held to the budget and a
/// little slack, so a module that overruns cannot keep the viewer waiting on it.
pub fn ask(resolve: &Resolver, ask: &Ask) -> anyhow::Result<Reply> {
    let secs = (Duration::from_millis(ask.budget_ms) + SLACK).as_secs() as u32;
    bounded(resolve, "watch-now", ask, secs)
}

pub fn reject(resolve: &Resolver, rejection: &Rejection) -> anyhow::Result<bool> {
    bounded(resolve, "watch-now-reject", rejection, SLACK.as_secs() as u32 * 2)
}

pub fn confirm(resolve: &Resolver, confirmation: &Confirmation) -> anyhow::Result<bool> {
    bounded(resolve, "watch-now-confirm", confirmation, SLACK.as_secs() as u32 * 2)
}

pub fn flag(resolve: &Resolver, flag: &Flag) -> anyhow::Result<bool> {
    bounded(resolve, "watch-now-flag", flag, SLACK.as_secs() as u32 * 2)
}

fn bounded<B: Serialize, T: serde::de::DeserializeOwned>(
    resolve: &Resolver,
    method: &str,
    body: &B,
    secs: u32,
) -> anyhow::Result<T> {
    let (base, token) =
        resolve().ok_or_else(|| anyhow::anyhow!("no module contributes this point"))?;
    let answer: Result<T, String> = kroma_http::Loopback::new()
        .max_time(secs)
        .header("authorization", format!("Bearer {token}"))
        .post_json(
            &format!("{base}/_port/{ACQUISITION}/{method}"),
            &serde_json::to_value(body)?,
        )?
        .ensure_ok()?
        .json()?;
    answer.map_err(|error| anyhow::anyhow!(error))
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
