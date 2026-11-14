use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(super) struct Envelope {
    pub status: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub error: Option<Failure>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Failure {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct User {
    pub username: String,
    pub is_premium: bool,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Magnet {
    pub filename: String,
    pub size: u64,
    pub status: String,
    pub status_code: u8,
    pub downloaded: u64,
    pub download_speed: u64,
    pub seeders: u32,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct Node {
    pub n: String,
    pub s: Option<u64>,
    pub l: Option<String>,
    pub e: Option<Vec<Node>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Unlocked {
    pub link: String,
    pub filesize: Option<u64>,
    pub delayed: Option<Value>,
}

pub(super) fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}
