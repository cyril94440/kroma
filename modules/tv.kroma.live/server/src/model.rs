use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::norm::{self, ChannelKey, Slot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Primary,
    Backup,
}

impl ProviderId {
    pub fn other(self) -> ProviderId {
        match self {
            ProviderId::Primary => ProviderId::Backup,
            ProviderId::Backup => ProviderId::Primary,
        }
    }

    pub fn prefix(self) -> char {
        match self {
            ProviderId::Primary => 'p',
            ProviderId::Backup => 's',
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Primary => "primary",
            ProviderId::Backup => "backup",
        }
    }

    pub fn parse(text: &str) -> Option<ProviderId> {
        match text {
            "primary" => Some(ProviderId::Primary),
            "backup" => Some(ProviderId::Backup),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Channel {
    pub provider: ProviderId,
    pub stream_id: i64,
    pub name: String,
    pub norm: String,
    pub compact: String,
    pub epg_id: Option<String>,
    pub group: String,
    pub url: Option<String>,
    pub is_4k: bool,
    pub key: ChannelKey,
    pub slot: Slot,
}

impl Channel {
    pub fn new(provider: ProviderId, stream_id: i64, name: &str, epg_id: Option<&str>, group: &str) -> Channel {
        let normalised = norm::fold(name);
        Channel {
            provider,
            stream_id,
            name: name.trim().to_string(),
            compact: norm::compact(&normalised),
            epg_id: epg_id.map(str::trim).filter(|id| !id.is_empty()).map(str::to_string),
            group: group.trim().to_string(),
            url: None,
            is_4k: normalised.contains("4k") || normalised.contains("uhd") || has_8k(&normalised),
            key: norm::channel_key(&normalised),
            slot: norm::family_slot(&normalised),
            norm: normalised,
        }
    }

    pub fn with_url(mut self, url: &str) -> Channel {
        self.url = Some(url.to_string());
        self
    }

    pub fn id(&self) -> String {
        format!("{}:{}", self.provider.prefix(), self.stream_id)
    }

    pub fn key_numbers(&self) -> BTreeSet<&str> {
        self.key.words.iter().filter(|w| w.bytes().all(|b| b.is_ascii_digit())).map(String::as_str).collect()
    }
}

fn has_8k(text: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| word == "8k")
}

pub fn parse_ref(reference: &str) -> Option<(ProviderId, i64)> {
    let (prefix, id) = reference.split_once(':')?;
    let provider = match prefix {
        "p" => ProviderId::Primary,
        "s" => ProviderId::Backup,
        _ => return None,
    };
    Some((provider, id.parse().ok()?))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Programme {
    pub channel_id: String,
    pub channel_name: String,
    pub title: String,
    pub description: String,
    pub start: i64,
    pub end: i64,
}

impl Programme {
    pub fn with_channel(mut self, channel_id: &str) -> Programme {
        self.channel_id = channel_id.to_string();
        self
    }
}
