use serde_json::{json, Value};

use kroma_module_sdk::domain::{Audience, NotificationSpec};

use super::Party;
use crate::chat_rules::clip;
use crate::model::Person;
use crate::store::{Idea, IdeaContext};

pub const GENRES: &[&str] = &["idea", "bug", "other"];
pub const STATUSES: &[&str] = &["received", "inProgress", "done", "refused"];
const TEXT_MAX: usize = 1000;
const REPLY_MAX: usize = 500;
const KEEP: usize = 500;
const GAP_MS: i64 = 10_000;
const PER_DAY: usize = 30;
const SHOWN: u32 = 200;
const CHAT_ATTACHED: u32 = 25;
const CHAT_ATTACHED_MS: i64 = 45 * 60_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdeaError {
    Empty,
    TooFast,
    TooMany,
    Unknown,
    BadStatus,
    Storage,
}

impl IdeaError {
    pub fn code(&self) -> &'static str {
        match self {
            IdeaError::Empty => "empty",
            IdeaError::TooFast => "tooFast",
            IdeaError::TooMany => "tooMany",
            IdeaError::Unknown => "unknown",
            IdeaError::BadStatus => "badStatus",
            IdeaError::Storage => "storage",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IdeaInput {
    pub genre: String,
    pub text: String,
    pub channel: Option<String>,
    pub device: Option<String>,
    pub version: Option<String>,
    pub browser: Option<String>,
    pub source: Option<String>,
}

pub fn public_view(i: &Idea) -> Value {
    json!({
        "id": i.id, "ts": i.ts, "genre": i.genre, "text": i.text, "status": i.status,
        "reply": i.reply, "author": { "userId": i.user_id, "name": i.name }, "updatedAt": i.updated_at,
    })
}

impl Party {
    pub fn ideas_view(&self) -> Vec<Value> {
        self.store.ideas(None, true, SHOWN).unwrap_or_default().iter().map(public_view).collect()
    }

    pub fn ideas_for_triage(&self, status: Option<&str>) -> Vec<Value> {
        self.store
            .ideas(status, false, 500)
            .unwrap_or_default()
            .iter()
            .map(|i| {
                let mut v = public_view(i);
                v["context"] = serde_json::to_value(&i.context).unwrap_or(Value::Null);
                v["source"] = json!(i.source);
                v
            })
            .collect()
    }

    fn attached_chat(&self, now: i64) -> Vec<Value> {
        self.store
            .recent_messages(CHAT_ATTACHED, None, Some(now - CHAT_ATTACHED_MS))
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.kind == crate::model::Kind::User)
            .map(|m| json!({ "ts": m.at, "name": clip(&m.name, 40), "text": clip(&m.text, 300) }))
            .collect()
    }

    pub fn submit_idea(&self, who: &Person, input: IdeaInput, internal: bool) -> Result<String, IdeaError> {
        let text = clip(&input.text, TEXT_MAX);
        if text.is_empty() {
            return Err(IdeaError::Empty);
        }
        let now = self.now();
        let mine = self.store.recent_idea_times(&who.user_id, now - 86_400_000).map_err(|_| IdeaError::Storage)?;
        if !internal && mine.iter().any(|t| now - t < GAP_MS) {
            return Err(IdeaError::TooFast);
        }
        if !internal && mine.len() >= PER_DAY {
            return Err(IdeaError::TooMany);
        }
        let has_context = input.channel.is_some() || input.device.is_some() || input.version.is_some() || input.browser.is_some();
        let context = has_context.then(|| IdeaContext {
            channel: input.channel.as_deref().map(|c| clip(c, 80)),
            device: input.device.as_deref().map(|c| clip(c, 40)),
            version: input.version.as_deref().map(|c| clip(c, 40)),
            browser: input.browser.as_deref().map(|c| clip(c, 120)),
            chat: self.attached_chat(now),
        });
        let id = {
            let mut st = self.lock();
            format!("{now:x}{:x}", {
                st.counter += 1;
                st.counter
            })
        };
        let idea = Idea {
            id: id.clone(),
            ts: now,
            user_id: who.user_id.clone(),
            name: who.name.clone(),
            genre: if GENRES.contains(&input.genre.as_str()) { input.genre } else { "other".into() },
            text,
            status: "received".into(),
            reply: None,
            updated_at: now,
            context,
            source: if internal { input.source.map(|s| clip(&s, 40)) } else { None },
        };
        self.store.insert_idea(&idea, KEEP).map_err(|_| IdeaError::Storage)?;
        self.publish_ideas();
        Ok(id)
    }

    fn publish_ideas(&self) {
        let ideas = self.ideas_view();
        let mut st = self.lock();
        self.emit(&mut st, "party.ideas", json!({ "ideas": ideas }));
    }

    pub fn update_idea(&self, id: &str, status: Option<&str>, reply: Option<&str>) -> Result<Value, IdeaError> {
        if status.is_some_and(|s| !STATUSES.contains(&s)) {
            return Err(IdeaError::BadStatus);
        }
        let now = self.now();
        let reply = reply.map(|r| Some(clip(r, REPLY_MAX)).filter(|r| !r.is_empty()));
        let changed = self
            .store
            .update_idea(id, status, reply.as_ref().map(Option::as_deref), now)
            .map_err(|_| IdeaError::Storage)?;
        if !changed {
            return Err(IdeaError::Unknown);
        }
        let idea = self.store.idea(id).ok().flatten().ok_or(IdeaError::Unknown)?;
        self.publish_ideas();
        if status.is_some() || reply.is_some() {
            let spec = NotificationSpec::declared("tv.kroma.party/idea.updated")
                .param("text", clip(&idea.text, 80));
            self.host.notify(&Audience::user(&idea.user_id), &spec);
        }
        Ok(public_view(&idea))
    }
}
