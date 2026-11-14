use std::fmt;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::settings::{ProviderKind, Settings};

mod gemini;
mod http;
mod openai;
pub mod screen;

pub use screen::ScreenReading;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolDecl {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
    pub malformed: Option<String>,
}

impl ToolCall {
    pub fn new(name: &str, args: Value) -> Self {
        Self { id: String::new(), name: name.into(), args, malformed: None }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allowed {
    Any,
    Only(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    NotConfigured,
    RateLimited,
    Transient(String),
    Rejected(String),
    Malformed(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::NotConfigured => f.write_str("the model is not configured"),
            ModelError::RateLimited => f.write_str("the model provider is rate limiting"),
            ModelError::Transient(m) => write!(f, "model call failed: {m}"),
            ModelError::Rejected(m) => write!(f, "model call rejected: {m}"),
            ModelError::Malformed(m) => write!(f, "model answer unreadable: {m}"),
        }
    }
}

impl std::error::Error for ModelError {}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct WebAnswer {
    pub text: String,
    pub sources: Vec<String>,
}

/// One conversation with a model for one turn: the model asks for tools, the
/// caller runs them and hands the results back.
pub trait Dialogue: Send {
    fn step(&mut self, allowed: &Allowed) -> Result<Vec<ToolCall>, ModelError>;
    fn answer(&mut self, results: &[(ToolCall, Value)]);
}

pub trait Model: Send + Sync {
    fn open(&self, system: &str, first: &str, tools: &[ToolDecl]) -> Box<dyn Dialogue>;
    fn models(&self) -> Result<Vec<String>, ModelError>;
    fn ping(&self) -> Result<String, ModelError>;
    fn web_search(&self, question: &str) -> Result<WebAnswer, ModelError>;
    fn read_screen(&self, jpeg_base64: &str, question: &str) -> Result<ScreenReading, ModelError>;
    fn can_search(&self) -> bool;
}

pub type ModelSource = Arc<dyn Fn() -> Option<Arc<dyn Model>> + Send + Sync>;

pub fn model_from(settings: &Settings) -> Option<Arc<dyn Model>> {
    if !settings.configured() {
        return None;
    }
    Some(match settings.provider {
        ProviderKind::Gemini => Arc::new(gemini::Gemini::new(settings)),
        ProviderKind::OpenAi => Arc::new(openai::OpenAi::new(settings)),
    })
}

#[cfg(test)]
pub(crate) mod tests;
