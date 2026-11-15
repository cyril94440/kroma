use std::sync::Mutex;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use kroma_module_sdk::host::HostCtx;
use kroma_module_sdk::http::Fetch;

pub const DEFAULT_STUN: &str = "stun:stun.l.google.com:19302";
const MINT_TTL_S: u32 = 86_400;
const REFRESH_AFTER_MS: i64 = 12 * 3_600_000;
const VALID_FOR_MS: i64 = MINT_TTL_S as i64 * 1000;
const RETRY_AFTER_FAILURE_MS: i64 = 60_000;
const MINT_SECONDS: u32 = 8;
const MINT_BYTES: u64 = 64 * 1024;
const PURPOSE: &str = "voice.turn";
const MAX_SERVERS: usize = 8;
const SCHEMES: [&str; 4] = ["stun:", "stuns:", "turn:", "turns:"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

pub trait TurnSource: Send + Sync {
    fn mint(&self, key_id: &str, api_token: &str, ttl_s: u32) -> Result<Vec<IceServer>>;
}

pub struct CloudflareTurn;

#[derive(Deserialize)]
#[serde(untagged)]
enum Urls {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize)]
struct Raw {
    urls: Urls,
    username: Option<String>,
    credential: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Minted {
    ice_servers: Vec<Raw>,
}

impl TurnSource for CloudflareTurn {
    fn mint(&self, key_id: &str, api_token: &str, ttl_s: u32) -> Result<Vec<IceServer>> {
        if !key_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            bail!("the key id is not a plain identifier");
        }
        let url = format!("https://rtc.live.cloudflare.com/v1/turn/keys/{key_id}/credentials/generate-ice-servers");
        let answer = Fetch::new()
            .purpose(PURPOSE)
            .https_only()
            .no_redirects()
            .max_time(MINT_SECONDS)
            .max_bytes(MINT_BYTES)
            .header("authorization", format!("Bearer {api_token}"))
            .post_json(&url, &json!({ "ttl": ttl_s }))?
            .ensure_ok()?;
        let minted: Minted = answer.json()?;
        let servers: Vec<IceServer> = minted
            .ice_servers
            .into_iter()
            .map(|raw| IceServer {
                urls: match raw.urls {
                    Urls::One(url) => vec![url],
                    Urls::Many(urls) => urls,
                },
                username: raw.username,
                credential: raw.credential,
            })
            .filter(|s| s.urls.iter().any(|u| has_scheme(u)))
            .take(MAX_SERVERS)
            .collect();
        if servers.is_empty() {
            bail!("no ice server in the answer");
        }
        Ok(servers)
    }
}

#[derive(Debug, Default)]
pub struct IceCache {
    servers: Vec<IceServer>,
    refresh_at: i64,
    expires_at: i64,
    retry_at: i64,
}

impl IceCache {
    fn usable(&self, now: i64) -> Option<Vec<IceServer>> {
        (!self.servers.is_empty() && now < self.expires_at).then(|| self.servers.clone())
    }
}

fn has_scheme(url: &str) -> bool {
    SCHEMES.iter().any(|s| url.starts_with(s))
}

fn urls(list: &str) -> Vec<String> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|u| has_scheme(u))
        .map(str::to_string)
        .collect()
}

fn static_servers(host: &dyn HostCtx) -> Vec<IceServer> {
    let mut servers = Vec::new();
    let stun = urls(&host.config_str("voiceStunUrls", DEFAULT_STUN));
    if !stun.is_empty() {
        servers.push(IceServer { urls: stun, username: None, credential: None });
    }
    let turn = urls(&host.config_str("voiceTurnUrls", ""));
    if !turn.is_empty() {
        let text = |key: &str| Some(host.config_str(key, "")).filter(|v| !v.is_empty());
        servers.push(IceServer { urls: turn, username: text("voiceTurnUsername"), credential: text("voiceTurnCredential") });
    }
    servers
}

fn minted(host: &dyn HostCtx, turn: &dyn TurnSource, cache: &Mutex<IceCache>, now: i64) -> Option<Vec<IceServer>> {
    let key_id = host.config_str("voiceCloudflareKeyId", "");
    let token = host.config_str("voiceCloudflareApiToken", "");
    if key_id.trim().is_empty() || token.trim().is_empty() {
        return None;
    }
    {
        let held = cache.lock().unwrap_or_else(|e| e.into_inner());
        if now < held.refresh_at || now < held.retry_at {
            return held.usable(now);
        }
    }
    let result = turn.mint(key_id.trim(), token.trim(), MINT_TTL_S);
    let mut held = cache.lock().unwrap_or_else(|e| e.into_inner());
    match result {
        Ok(servers) => {
            held.servers.clone_from(&servers);
            held.refresh_at = now + REFRESH_AFTER_MS;
            held.expires_at = now + VALID_FOR_MS;
            held.retry_at = 0;
            Some(servers)
        }
        Err(_) => {
            tracing::warn!("turn credentials could not be minted, falling back to the configured servers");
            held.retry_at = now + RETRY_AFTER_FAILURE_MS;
            held.usable(now)
        }
    }
}

/// The ICE servers a participant hands to `RTCPeerConnection`. Time-limited
/// Cloudflare credentials win over a static list, a failed mint falls back to
/// the static list (or STUN alone), and none of it is ever logged.
pub fn ice_servers(host: &dyn HostCtx, turn: &dyn TurnSource, cache: &Mutex<IceCache>, now: i64) -> Vec<IceServer> {
    minted(host, turn, cache, now).unwrap_or_else(|| static_servers(host))
}
