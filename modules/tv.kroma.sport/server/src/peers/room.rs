use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use kroma_module_sdk::host::{call_point, pinned_resolver, HostCtx};

pub const ROOM: &str = "tv.kroma.party/room";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SwitchOutcome {
    pub outcome: String,
    pub vote_id: Option<String>,
    pub reason: Option<String>,
}

pub trait Room: Send + Sync {
    fn say(&self, text: &str, card: Option<Value>) -> Result<String>;
    fn request_switch(
        &self,
        channel_id: &str,
        requested_by: &str,
        reason: &str,
    ) -> Result<SwitchOutcome>;
}

pub struct PeerRoom {
    host: Arc<dyn HostCtx>,
}

impl PeerRoom {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host }
    }

    fn ask<T: serde::de::DeserializeOwned>(&self, method: &str, body: &Value) -> Result<T> {
        let resolve = pinned_resolver(&*self.host, ROOM, None)
            .ok_or_else(|| anyhow!("no module contributes {ROOM}"))?;
        call_point(&resolve, ROOM, method, body)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Said {
    id: String,
}

impl Room for PeerRoom {
    fn say(&self, text: &str, card: Option<Value>) -> Result<String> {
        Ok(self
            .ask::<Said>(
                "say",
                &json!({"text": text, "as": "system", "card": card, "replyTo": null}),
            )?
            .id)
    }

    fn request_switch(
        &self,
        channel_id: &str,
        requested_by: &str,
        reason: &str,
    ) -> Result<SwitchOutcome> {
        self.ask(
            "requestSwitch",
            &json!({"channelId": channel_id, "requestedBy": requested_by, "reason": reason}),
        )
    }
}
