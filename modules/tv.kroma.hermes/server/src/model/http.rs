use kroma_module_sdk::http::{Fetch, Response};
use serde_json::Value;

use super::ModelError;
use crate::blackbox::{clip, mask};

pub struct Endpoint {
    pub purpose: &'static str,
    pub timeout_s: u32,
    pub headers: Vec<(String, String)>,
    pub secrets: Vec<String>,
}

impl Endpoint {
    fn fetch(&self) -> Fetch {
        self.headers.iter().fold(Fetch::new().purpose(self.purpose).max_time(self.timeout_s), |f, (k, v)| f.header(k, v.clone()))
    }

    fn failure(&self, text: &str) -> String {
        clip(&mask(text, &self.secrets), 240)
    }

    fn settle(&self, sent: anyhow::Result<Response>) -> Result<Value, ModelError> {
        let response = sent.map_err(|e| ModelError::Transient(self.failure(&format!("{e:#}"))))?;
        match response.status {
            200..=299 => serde_json::from_slice(&response.body)
                .map_err(|e| ModelError::Malformed(self.failure(&format!("{e}: {}", response.text())))),
            429 => Err(ModelError::RateLimited),
            408 | 500..=599 => Err(ModelError::Transient(self.failure(&format!("HTTP {}: {}", response.status, response.text())))),
            status => Err(ModelError::Rejected(self.failure(&format!("HTTP {status}: {}", response.text())))),
        }
    }

    pub fn post(&self, url: &str, body: &Value) -> Result<Value, ModelError> {
        let first = self.settle(self.fetch().post_json(url, body));
        match first {
            Err(ModelError::Transient(_)) => self.settle(self.fetch().post_json(url, body)),
            other => other,
        }
    }

    pub fn get(&self, url: &str) -> Result<Value, ModelError> {
        self.settle(self.fetch().get(url))
    }
}
