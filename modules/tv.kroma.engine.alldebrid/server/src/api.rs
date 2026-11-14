//! The AllDebrid HTTP API: one method per endpoint this module calls, each
//! answering the envelope's `data` or the failure it carried.

use std::fmt;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use kroma_module_sdk::http::{Fetch, FormPart, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

mod wire;

pub use wire::{Magnet, Node, Unlocked, User};

pub(crate) const DEFAULT_BASE: &str = "https://api.alldebrid.com";
const PURPOSE: &str = "download-client";
const CALL_SECS: u32 = 60;
const UNKNOWN_MAGNET: &str = "MAGNET_INVALID_ID";

#[derive(Debug, Clone)]
pub struct Api {
    base: String,
    key: String,
}

/// An error AllDebrid itself reported, as its `code` and `message`.
#[derive(Debug)]
pub struct Refused {
    pub code: String,
    pub message: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code.as_str() {
            "AUTH_BAD_APIKEY" | "AUTH_MISSING_APIKEY" => {
                write!(f, "AllDebrid rejected the API key ({})", self.code)
            }
            _ => write!(f, "AllDebrid: {} ({})", self.message, self.code),
        }
    }
}

impl std::error::Error for Refused {}

pub fn is_unknown_magnet(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<Refused>()
        .is_some_and(|refused| refused.code == UNKNOWN_MAGNET)
}

impl Api {
    pub fn new(base_override: &str, key: &str) -> Result<Self> {
        let key = key.trim();
        if key.is_empty() {
            bail!("an AllDebrid API key is required");
        }
        let base = base_override.trim().trim_end_matches('/');
        Ok(Self {
            base: if base.is_empty() { DEFAULT_BASE } else { base }.to_string(),
            key: key.to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn fetch(&self) -> Fetch {
        Fetch::new()
            .purpose(PURPOSE)
            .max_time(CALL_SECS)
            .header("authorization", format!("Bearer {}", self.key))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<Value> {
        let fetch = query
            .iter()
            .fold(self.fetch(), |fetch, (name, value)| fetch.query(name, *value));
        data_of(fetch.get(&self.url(path)))
    }

    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Result<Value> {
        data_of(self.fetch().post_form(&self.url(path), fields))
    }

    pub fn user(&self) -> Result<User> {
        let data = self.get("/v4/user", &[])?;
        parse(data.get("user"), "user")
    }

    pub fn upload_magnet(&self, magnet: &str) -> Result<String> {
        let data = self.post("/v4/magnet/upload", &[("magnets[]", magnet)])?;
        uploaded(&data, "magnets")
    }

    pub fn upload_torrent(&self, file: &Path) -> Result<String> {
        let parts = [("files[]", FormPart::File(file))];
        let data = data_of(self.fetch().post_multipart(&self.url("/v4/magnet/upload/file"), &parts))?;
        uploaded(&data, "files")
    }

    pub fn magnet(&self, id: &str) -> Result<Option<Magnet>> {
        let data = match self.get("/v4.1/magnet/status", &[("id", id)]) {
            Err(error) if is_unknown_magnet(&error) => return Ok(None),
            other => other?,
        };
        let first = match data.get("magnets") {
            Some(Value::Array(all)) => all.first(),
            Some(single @ Value::Object(_)) => Some(single),
            _ => None,
        };
        first.map(|magnet| parse(Some(magnet), "magnet")).transpose()
    }

    pub fn files(&self, id: &str) -> Result<Vec<Node>> {
        let data = self.post("/v4/magnet/files", &[("id[]", id)])?;
        let entry = match data.get("magnets") {
            Some(Value::Array(all)) => all.first(),
            Some(single @ Value::Object(_)) => Some(single),
            _ => None,
        }
        .ok_or_else(|| anyhow!("AllDebrid listed no files for this magnet"))?;
        if let Some(refused) = entry.get("error").and_then(failure) {
            return Err(refused.into());
        }
        parse(entry.get("files"), "files")
    }

    pub fn unlock(&self, link: &str) -> Result<Unlocked> {
        let data = self.get("/v4/link/unlock", &[("link", link)])?;
        let unlocked: Unlocked = serde_json::from_value(data).context("read the unlocked link")?;
        if unlocked.link.is_empty() {
            if unlocked.delayed.is_some() {
                bail!("AllDebrid delayed this link, which is not supported");
            }
            bail!("AllDebrid returned no download link");
        }
        Ok(unlocked)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        match self.post("/v4/magnet/delete", &[("id", id)]) {
            Err(error) if is_unknown_magnet(&error) => Ok(()),
            other => other.map(|_| ()),
        }
    }
}

fn failure(value: &Value) -> Option<Refused> {
    let code = value.get("code")?.as_str()?.to_string();
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some(Refused { code, message })
}

fn parse<T: DeserializeOwned>(value: Option<&Value>, what: &str) -> Result<T> {
    let value = value.ok_or_else(|| anyhow!("AllDebrid's reply has no {what}"))?;
    serde_json::from_value(value.clone()).with_context(|| format!("read AllDebrid's {what}"))
}

fn uploaded(data: &Value, list: &str) -> Result<String> {
    let entry = data
        .get(list)
        .and_then(Value::as_array)
        .and_then(|all| all.first())
        .ok_or_else(|| anyhow!("AllDebrid accepted nothing"))?;
    if let Some(refused) = entry.get("error").and_then(failure) {
        return Err(refused.into());
    }
    entry
        .get("id")
        .and_then(wire::id_text)
        .ok_or_else(|| anyhow!("AllDebrid returned no magnet id"))
}

fn data_of(sent: Result<Response>) -> Result<Value> {
    let response = sent?;
    let status = response.status;
    let Ok(envelope) = response.json::<wire::Envelope>() else {
        bail!("AllDebrid answered HTTP {status} with something that is not its API");
    };
    if envelope.status == "success" {
        return Ok(envelope.data);
    }
    let error = envelope.error.unwrap_or_default();
    Err(Refused {
        code: error.code,
        message: error.message,
    }
    .into())
}

#[cfg(test)]
mod tests;
