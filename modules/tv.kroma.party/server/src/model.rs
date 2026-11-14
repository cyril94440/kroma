use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub user_id: String,
    pub name: String,
}

impl Person {
    pub fn new(user_id: &str, name: &str) -> Self {
        Self {
            user_id: user_id.into(),
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    User,
    System,
    Agent,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::System => "system",
            Kind::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Kind {
        match s {
            "system" => Kind::System,
            "agent" => Kind::Agent,
            _ => Kind::User,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChatMessage {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub kind: Kind,
    pub text: String,
    pub key: Option<String>,
    pub params: Map<String, Value>,
    pub action: Option<String>,
    pub card: Option<Value>,
    pub reply_to: Option<String>,
    pub at: i64,
    pub reactions: BTreeMap<String, Vec<Person>>,
}

impl Default for ChatMessage {
    fn default() -> Self {
        Self {
            id: String::new(),
            user_id: None,
            name: String::new(),
            kind: Kind::User,
            text: String::new(),
            key: None,
            params: Map::new(),
            action: None,
            card: None,
            reply_to: None,
            at: 0,
            reactions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomMessage {
    pub id: String,
    pub user_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub text: String,
    pub at: i64,
}

impl From<&ChatMessage> for RoomMessage {
    fn from(m: &ChatMessage) -> Self {
        Self {
            id: m.id.clone(),
            user_id: m.user_id.clone(),
            name: m.name.clone(),
            kind: m.kind.as_str().into(),
            text: m.text.clone(),
            at: m.at,
        }
    }
}

pub fn params(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
        .collect()
}

pub fn text_vars(map: &Map<String, Value>) -> Vec<(&str, &str)> {
    map.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.as_str(), s)))
        .collect()
}
