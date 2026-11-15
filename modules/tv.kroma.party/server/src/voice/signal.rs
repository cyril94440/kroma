use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::error::VoiceError;
use super::limits::{valid_token, MAX_CANDIDATE_BYTES, MAX_SDP_BYTES, MAX_TOKEN_BYTES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SignalKind {
    Description,
    Candidate,
    Hello,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct Description {
    #[serde(rename = "type")]
    pub kind: String,
    pub sdp: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Candidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u32>,
    pub username_fragment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signal {
    pub from: String,
    pub to: String,
    pub session: String,
    pub kind: SignalKind,
    #[serde(default)]
    pub description: Option<Description>,
    #[serde(default)]
    pub candidate: Option<Candidate>,
}

fn short(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|s| s.len() <= MAX_TOKEN_BYTES)
}

impl Signal {
    pub fn validate(&self) -> Result<(), VoiceError> {
        if !valid_token(&self.from) || !valid_token(&self.to) || !valid_token(&self.session) {
            return Err(VoiceError::BadSignal);
        }
        match self.kind {
            SignalKind::Description => {
                let d = self.description.as_ref().ok_or(VoiceError::BadSignal)?;
                if self.candidate.is_some()
                    || !matches!(d.kind.as_str(), "offer" | "answer")
                    || d.sdp.is_empty()
                {
                    return Err(VoiceError::BadSignal);
                }
                if d.sdp.len() > MAX_SDP_BYTES {
                    return Err(VoiceError::TooLarge);
                }
            }
            SignalKind::Candidate => {
                let c = self.candidate.as_ref().ok_or(VoiceError::BadSignal)?;
                if self.description.is_some() {
                    return Err(VoiceError::BadSignal);
                }
                if c.candidate.len() > MAX_CANDIDATE_BYTES
                    || !short(&c.sdp_mid)
                    || !short(&c.username_fragment)
                {
                    return Err(VoiceError::TooLarge);
                }
            }
            SignalKind::Hello => {
                if self.description.is_some() || self.candidate.is_some() {
                    return Err(VoiceError::BadSignal);
                }
            }
        }
        Ok(())
    }

    pub fn payload(&self, from_user: &str, n: u64) -> Value {
        let mut payload = json!({
            "to": self.to,
            "from": self.from,
            "fromUserId": from_user,
            "session": self.session,
            "n": n,
            "kind": self.kind,
        });
        if let Some(description) = &self.description {
            payload["description"] = json!(description);
        }
        if let Some(candidate) = &self.candidate {
            payload["candidate"] = json!(candidate);
        }
        payload
    }
}
