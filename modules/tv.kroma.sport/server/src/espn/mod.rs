use anyhow::{bail, Result};
use kroma_module_sdk::http::Fetch;
use serde_json::Value;

pub const PURPOSE: &str = "espn";
const CALL_SECS: u32 = 8;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

pub fn scoreboard(code: &str, day: &str) -> String {
    format!("/apis/site/v2/sports/soccer/{code}/scoreboard?dates={day}")
}

pub fn summary(code: &str, event_id: &str) -> String {
    format!("/apis/site/v2/sports/soccer/{code}/summary?event={event_id}")
}

pub fn standings(code: &str) -> String {
    format!("/apis/v2/sports/soccer/{code}/standings")
}

pub fn live_scores(code: &str) -> String {
    format!("/apis/site/v2/sports/soccer/{code}/scoreboard")
}

pub trait Espn: Send + Sync {
    fn get(&self, path: &str) -> Result<Value>;
}

pub struct HttpEspn {
    base: Box<dyn Fn() -> String + Send + Sync>,
}

impl HttpEspn {
    pub fn new(base: impl Fn() -> String + Send + Sync + 'static) -> Self {
        Self {
            base: Box::new(base),
        }
    }
}

impl Espn for HttpEspn {
    fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}{path}", (self.base)());
        let response = Fetch::new()
            .purpose(PURPOSE)
            .max_time(CALL_SECS)
            .max_bytes(MAX_BYTES)
            .header("user-agent", "Mozilla/5.0")
            .get(&url)?;
        if response.status != 200 {
            bail!("ESPN answered HTTP {}", response.status);
        }
        response.json()
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
pub mod fake;
#[cfg(test)]
mod tests;
