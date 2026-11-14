//! The AllDebrid HTTP API: one method per endpoint this module calls, each
//! answering the envelope's `data` or the failure it carried.

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use kroma_module_sdk::http::{Fetch, FormPart, Response};
use serde::de::DeserializeOwned;
use serde_json::Value;

mod errors;
mod wire;

pub use errors::{is_retry_later, is_unknown_magnet, Delayed, Refused, RetryLater};
pub use wire::{Magnet, Node, Unlocked, Uploaded, User};

pub(crate) const DEFAULT_BASE: &str = "https://api.alldebrid.com";
const PURPOSE: &str = "download-client";
const CALL_SECS: u32 = 60;

#[derive(Debug, Clone)]
pub struct Api {
    base: String,
    key: String,
    call_secs: u32,
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
            call_secs: CALL_SECS,
        })
    }

    /// The same account, with no call allowed to outlast `left`.
    pub fn within(&self, left: std::time::Duration) -> Self {
        Self {
            call_secs: (left.as_secs() as u32).clamp(1, CALL_SECS),
            ..self.clone()
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn fetch(&self) -> Fetch {
        Fetch::new()
            .purpose(PURPOSE)
            .max_time(self.call_secs)
            .header("authorization", format!("Bearer {}", self.key))
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn get(&self, path: &str) -> Result<Value> {
        errors::check(&self.base, Instant::now())?;
        self.settled(self.fetch().get(&self.url(path)))
    }

    fn post(&self, path: &str, fields: &[(&str, &str)]) -> Result<Value> {
        errors::check(&self.base, Instant::now())?;
        self.settled(self.fetch().post_form(&self.url(path), fields))
    }

    fn settled(&self, sent: Result<Response>) -> Result<Value> {
        match data_of(&self.base, sent) {
            Err(error) if is_retry_later(&error) => Err(error),
            other => {
                errors::answered(&self.base);
                other
            }
        }
    }

    pub fn user(&self) -> Result<User> {
        let data = self.get("/v4/user")?;
        parse(data.get("user"), "user")
    }

    pub fn upload_magnet(&self, magnet: &str) -> Result<String> {
        let data = self.post("/v4/magnet/upload", &[("magnets[]", magnet)])?;
        uploaded(&data, "magnets")
    }

    pub fn upload_magnets(&self, magnets: &[&str]) -> Result<Vec<Result<Uploaded>>> {
        let fields: Vec<(&str, &str)> = magnets
            .iter()
            .map(|magnet| ("magnets[]", *magnet))
            .collect();
        let data = self.post("/v4/magnet/upload", &fields)?;
        Ok(entries(&data, "magnets")
            .iter()
            .map(entry_uploaded)
            .collect())
    }

    pub fn upload_torrent_entry(&self, file: &Path) -> Result<Uploaded> {
        let parts = [("files[]", FormPart::File(file))];
        errors::check(&self.base, Instant::now())?;
        let data = self.settled(
            self.fetch()
                .post_multipart(&self.url("/v4/magnet/upload/file"), &parts),
        )?;
        entries(&data, "files")
            .first()
            .map(entry_uploaded)
            .unwrap_or_else(|| Err(anyhow!("AllDebrid accepted nothing")))
    }

    pub fn magnet(&self, id: &str) -> Result<Option<Magnet>> {
        let data = match self.post("/v4.1/magnet/status", &[("id", id)]) {
            Err(error) if is_unknown_magnet(&error) => return Ok(None),
            other => other?,
        };
        magnet_list(&data)
            .first()
            .map(|magnet| parse(Some(magnet), "magnet"))
            .transpose()
    }

    /// Ids of every magnet in the account, from one `magnet/status` call with no `id`.
    pub fn magnet_ids(&self) -> Result<HashSet<String>> {
        let data = self.post("/v4.1/magnet/status", &[])?;
        if data.get("magnets").is_none() {
            bail!("AllDebrid's reply has no magnet list");
        }
        Ok(magnet_list(&data)
            .iter()
            .filter_map(|magnet| magnet.get("id").and_then(wire::id_text))
            .collect())
    }

    pub fn files(&self, id: &str) -> Result<Vec<Node>> {
        let data = self.post("/v4/magnet/files", &[("id[]", id)])?;
        let entry = magnet_list(&data)
            .first()
            .ok_or_else(|| anyhow!("AllDebrid listed no files for this magnet"))?;
        if let Some(refused) = entry.get("error").and_then(failure) {
            return Err(refused.into());
        }
        parse(entry.get("files"), "files")
    }

    pub fn unlock(&self, link: &str) -> Result<Unlocked> {
        let data = self.post("/v4/link/unlock", &[("link", link)])?;
        let unlocked: Unlocked = serde_json::from_value(data).context("read the unlocked link")?;
        if unlocked.link.is_empty() {
            if let Some(id) = unlocked.delayed.as_ref().and_then(wire::id_text) {
                return Err(Delayed { id }.into());
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

fn entries<'a>(data: &'a Value, list: &str) -> &'a [Value] {
    match data.get(list) {
        Some(Value::Array(all)) => all.as_slice(),
        Some(single @ Value::Object(_)) => std::slice::from_ref(single),
        _ => &[],
    }
}

fn magnet_list(data: &Value) -> &[Value] {
    entries(data, "magnets")
}

fn entry_uploaded(entry: &Value) -> Result<Uploaded> {
    if let Some(refused) = entry.get("error").and_then(failure) {
        return Err(refused.into());
    }
    let id = entry
        .get("id")
        .and_then(wire::id_text)
        .ok_or_else(|| anyhow!("AllDebrid returned no magnet id"))?;
    let ready = entry.get("ready").and_then(Value::as_bool).unwrap_or(false);
    Ok(Uploaded { id, ready })
}

fn uploaded(data: &Value, list: &str) -> Result<String> {
    let entry = entries(data, list)
        .first()
        .ok_or_else(|| anyhow!("AllDebrid accepted nothing"))?;
    entry_uploaded(entry).map(|uploaded| uploaded.id)
}

fn data_of(base: &str, sent: Result<Response>) -> Result<Value> {
    let response = sent?;
    let status = response.status;
    if matches!(status, 429 | 503) {
        return Err(errors::limited(base, status, Instant::now()).into());
    }
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
