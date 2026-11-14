use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{bail, Result};
use kroma_module_sdk::http::Fetch;

use crate::catalogue::Catalogue;
use crate::config::Settings;
use crate::db::{self, Pool};
use crate::loader;
use crate::model::{Channel, ProviderId};
use crate::portal::{self, redact, Portal};
use crate::rules::Rules;

pub const TTL_MS: i64 = 3_600_000;
pub const RETRY_AFTER_FAILURE_MS: i64 = 60_000;
const API_BYTES: u64 = 64 * 1024 * 1024;
const API_SECONDS: u32 = 60;
const PURPOSE: &str = "provider";

pub struct Providers {
    pool: Pool,
    held: Mutex<HashMap<ProviderId, Arc<Catalogue>>>,
    retry_at: Mutex<HashMap<ProviderId, i64>>,
}

fn fetch(user_agent: &str, seconds: u32, bytes: u64) -> Fetch {
    Fetch::new().purpose(PURPOSE).max_time(seconds).max_bytes(bytes).header("user-agent", user_agent)
}

impl Providers {
    pub fn new(pool: Pool) -> Providers {
        Providers { pool, held: Mutex::new(HashMap::new()), retry_at: Mutex::new(HashMap::new()) }
    }

    pub fn catalogue(&self, provider: ProviderId) -> Arc<Catalogue> {
        if let Some(held) = self.held.lock().unwrap_or_else(PoisonError::into_inner).get(&provider) {
            return held.clone();
        }
        let loaded = Arc::new(Catalogue::new(db::load_channels(&self.pool, provider).unwrap_or_default()));
        self.held.lock().unwrap_or_else(PoisonError::into_inner).insert(provider, loaded.clone());
        loaded
    }

    pub fn is_due(&self, provider: ProviderId, now_ms: i64) -> bool {
        let state = db::catalogue_state(&self.pool, provider).unwrap_or_default();
        let retry = self.retry_at.lock().unwrap_or_else(PoisonError::into_inner).get(&provider).copied().unwrap_or(0);
        now_ms >= retry && (state.channels == 0 || now_ms - state.fetched_at >= TTL_MS)
    }

    pub fn refresh(&self, provider: ProviderId, settings: &Settings, rules: &Rules, now_ms: i64) -> Result<usize> {
        let url = settings.url_of(provider);
        if url.is_empty() {
            bail!("no {} provider is configured", provider.as_str());
        }
        match self.download(provider, url, settings.user_agent_of(provider), rules) {
            Ok(channels) => {
                db::replace_channels(&self.pool, provider, &channels, now_ms)?;
                let count = channels.len();
                self.held.lock().unwrap_or_else(PoisonError::into_inner).insert(provider, Arc::new(Catalogue::new(channels)));
                self.retry_at.lock().unwrap_or_else(PoisonError::into_inner).remove(&provider);
                Ok(count)
            }
            Err(error) => {
                let message = redact(&format!("{error:#}"));
                let _ = db::note_catalogue_failure(&self.pool, provider, &message, now_ms);
                self.retry_at.lock().unwrap_or_else(PoisonError::into_inner).insert(provider, now_ms + RETRY_AFTER_FAILURE_MS);
                bail!("{message}")
            }
        }
    }

    fn download(&self, provider: ProviderId, url: &str, user_agent: &str, rules: &Rules) -> Result<Vec<Channel>> {
        if portal::is_playlist_file(url) {
            let body = fetch(user_agent, API_SECONDS, API_BYTES).get(url)?.ensure_ok()?;
            let text = body.text();
            if !text.trim_start().starts_with("#EXTM3U") {
                bail!("the playlist does not start with #EXTM3U");
            }
            return Ok(loader::parse_m3u(provider, &text, rules));
        }
        let portal = Portal::parse(url)?;
        let groups = fetch(user_agent, API_SECONDS, API_BYTES)
            .get(&portal.api_url(Some("get_live_categories")))
            .ok()
            .filter(|r| (200..300).contains(&r.status))
            .map(|r| loader::categories(&r.body))
            .unwrap_or_default();
        let response = fetch(user_agent, API_SECONDS, API_BYTES).get(&portal.api_url(Some("get_live_streams")))?.ensure_ok()?;
        let channels = loader::parse_xtream(provider, &response.body, &groups, rules)?;
        if channels.is_empty() {
            bail!("the portal answered no channel");
        }
        Ok(channels)
    }

    pub fn ping(&self, provider: ProviderId, settings: &Settings) -> bool {
        let Some(portal) = settings.portal(provider) else { return false };
        let Ok(response) = fetch(settings.user_agent_of(provider), 8, 1024 * 1024).get(&portal.api_url(None)) else { return false };
        response.status == 200
            && response.json::<serde_json::Value>().ok().and_then(|v| v.pointer("/user_info/status").and_then(|s| s.as_str().map(|s| s.eq_ignore_ascii_case("active")))).unwrap_or(false)
    }

    pub fn stream_url(&self, channel: &Channel, settings: &Settings) -> Option<String> {
        channel.url.clone().or_else(|| settings.portal(channel.provider).map(|p| p.stream_url(channel.stream_id)))
    }

    pub fn find_by_ref(&self, reference: &str) -> Option<Channel> {
        let (provider, id) = crate::model::parse_ref(reference)?;
        self.catalogue(provider).by_stream_id(id).cloned()
    }

    pub fn mapped(&self, channel: &Channel, to: ProviderId, rules: &Rules) -> Option<Channel> {
        let source = self.catalogue(channel.provider);
        self.catalogue(to).equivalents(&channel.name, Some(&source), 1, rules).first().map(|c| (*c).clone())
    }

    pub fn forget(&self) {
        self.held.lock().unwrap_or_else(PoisonError::into_inner).clear();
    }
}
