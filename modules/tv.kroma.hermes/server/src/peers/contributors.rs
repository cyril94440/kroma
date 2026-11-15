use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use kroma_module_sdk::host::{call_point, HostCtx};

use crate::model::ToolDecl;

pub const TOOLS: &str = "tv.kroma.hermes/tools";
const CACHE_MS: i64 = 30_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ExternalTool {
    pub module_id: String,
    pub decl: ToolDecl,
}

/// The tools other modules contribute to the assistant, through the point this
/// module defines.
pub trait Contributors: Send + Sync {
    fn tools(&self, now: i64) -> Vec<ExternalTool>;
    fn call(&self, module_id: &str, name: &str, arguments: &Value, user_id: &str, message_id: &str) -> Result<Value>;
}

const LIST: &str = "list";
const CALL: &str = "call";

pub const METHODS: [&str; 2] = [LIST, CALL];

pub struct PeerContributors {
    host: Arc<dyn HostCtx>,
    cache: Mutex<Option<(i64, Vec<ExternalTool>)>>,
}

impl PeerContributors {
    pub fn new(host: Arc<dyn HostCtx>) -> Self {
        Self { host, cache: Mutex::new(None) }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Listed {
    tools: Vec<Declared>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Declared {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Answered {
    result: Value,
}

impl Contributors for PeerContributors {
    fn tools(&self, now: i64) -> Vec<ExternalTool> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, tools)) = cache.as_ref() {
            if now - at < CACHE_MS {
                return tools.clone();
            }
        }
        let mut found = Vec::new();
        for contribution in self.host.contributions(TOOLS) {
            let listed: Result<Listed> = call_point(&contribution.resolver(), TOOLS, LIST, &json!({}));
            let Ok(listed) = listed else { continue };
            for tool in listed.tools.into_iter().filter(|t| !t.name.is_empty()) {
                let parameters = if tool.parameters.is_object() { tool.parameters } else { json!({ "type": "object", "properties": {} }) };
                found.push(ExternalTool {
                    module_id: contribution.module_id.clone(),
                    decl: ToolDecl { name: tool.name, description: tool.description, parameters },
                });
            }
        }
        *cache = Some((now, found.clone()));
        found
    }

    fn call(&self, module_id: &str, name: &str, arguments: &Value, user_id: &str, message_id: &str) -> Result<Value> {
        let contribution = self
            .host
            .contributions(TOOLS)
            .into_iter()
            .find(|c| c.module_id == module_id)
            .ok_or_else(|| anyhow!("{module_id} no longer contributes tools"))?;
        let answered: Answered = call_point(
            &contribution.resolver(),
            TOOLS,
            CALL,
            &json!({ "name": name, "arguments": arguments, "context": { "userId": user_id, "messageId": message_id } }),
        )?;
        Ok(answered.result)
    }
}
