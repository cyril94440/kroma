use serde_json::{json, Value};

use super::{Actor, Party};
use crate::chat_rules as rules;
use crate::model::{ChatMessage, Kind, Person, RoomMessage};
use crate::votewords::parse_vote;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatError {
    Empty,
    RateLimited,
    Unknown,
}

impl ChatError {
    pub fn code(&self) -> &'static str {
        match self {
            ChatError::Empty => "empty",
            ChatError::RateLimited => "rateLimited",
            ChatError::Unknown => "unknown",
        }
    }
}

pub(super) fn message_event(m: &ChatMessage, agent_name: &str) -> Value {
    let mut v = serde_json::to_value(m).unwrap_or(Value::Null);
    if m.kind == Kind::User {
        v["mentionsAgent"] = json!(rules::mentions_agent(&m.text, agent_name));
        v["isQuestion"] = json!(rules::is_question(&m.text));
        v["keywordHit"] = json!(rules::keyword_hit(&m.text));
    }
    v
}

fn allowed(window: &mut Vec<i64>, now: i64, count: usize, span: i64) -> bool {
    window.retain(|t| now - t < span);
    if window.len() >= count {
        return false;
    }
    window.push(now);
    true
}

impl Party {
    pub fn post_message(&self, actor: &Actor, text: &str) -> Result<ChatMessage, ChatError> {
        let text = rules::clip(text, rules::TEXT_MAX);
        if text.is_empty() {
            return Err(ChatError::Empty);
        }
        let now = self.now();
        let message = {
            let mut st = self.lock();
            let window = st.rate.entry(actor.person.user_id.clone()).or_default();
            if !allowed(window, now, rules::RATE_COUNT, rules::RATE_WINDOW_MS) {
                tracing::warn!(user = %actor.person.user_id, "chat rate limit hit");
                return Err(ChatError::RateLimited);
            }
            let m = ChatMessage {
                user_id: Some(actor.person.user_id.clone()),
                name: actor.person.name.clone(),
                kind: Kind::User,
                text: text.clone(),
                at: now,
                ..ChatMessage::default()
            };
            self.record_message(&mut st, m)
        };
        self.after_user_message(&actor.person, &text);
        Ok(message)
    }

    fn after_user_message(&self, who: &Person, text: &str) {
        if let Some(approve) = parse_vote(text) {
            let open = self.lock().vote.as_ref().is_some_and(|v| v.accepts_voter(&who.user_id));
            if open {
                let _ = self.cast_vote(who, approve);
            }
        }
        self.poll_from_chat(who, text);
    }

    pub fn say(&self, text: &str, as_agent: bool, card: Option<Value>, reply_to: Option<String>) -> Result<String, ChatError> {
        let text = rules::clip(text, rules::AGENT_TEXT_MAX);
        if text.is_empty() {
            return Err(ChatError::Empty);
        }
        let now = self.now();
        let name = if as_agent { self.agent_name() } else { String::new() };
        let mut st = self.lock();
        let m = ChatMessage {
            kind: if as_agent { Kind::Agent } else { Kind::System },
            name,
            text,
            card,
            reply_to,
            at: now,
            ..ChatMessage::default()
        };
        Ok(self.record_message(&mut st, m).id)
    }

    pub fn history(&self, limit: u32, before: Option<i64>) -> Vec<ChatMessage> {
        self.store
            .recent_messages(limit.clamp(1, 500), before, None)
            .unwrap_or_default()
    }

    pub fn room_history(&self, limit: u32, before: Option<i64>) -> Vec<RoomMessage> {
        self.history(limit, before).iter().map(RoomMessage::from).collect()
    }

    pub fn toggle_reaction(&self, actor: &Actor, message_id: &str, emoji: &str) -> Result<(), ChatError> {
        if !rules::REACTION_EMOJIS.contains(&emoji) || message_id.is_empty() {
            return Err(ChatError::Unknown);
        }
        let in_window = self
            .store
            .within_reaction_window(message_id, rules::REACTION_WINDOW)
            .unwrap_or(false);
        let found = if in_window { self.store.message(message_id).ok().flatten() } else { None };
        let Some(mut m) = found else {
            return Err(ChatError::Unknown);
        };
        let who = &actor.person;
        let people = m.reactions.entry(emoji.to_string()).or_default();
        match people.iter().position(|p| p.user_id == who.user_id) {
            Some(i) => {
                people.remove(i);
            }
            None => people.push(who.clone()),
        }
        if people.is_empty() {
            m.reactions.remove(emoji);
        }
        if let Err(e) = self.store.save_reactions(message_id, &m) {
            tracing::error!(error = %e, "reactions not stored");
            return Err(ChatError::Unknown);
        }
        let mut st = self.lock();
        self.emit(&mut st, "party.reactions", json!({ "id": m.id, "reactions": m.reactions }));
        Ok(())
    }

    pub fn floating_reaction(&self, actor: &Actor, emoji: &str) -> Result<(), ChatError> {
        let emoji: String = emoji.trim().chars().take(rules::FLOATING_EMOJI_MAX).collect();
        if emoji.is_empty() {
            return Err(ChatError::Empty);
        }
        let now = self.now();
        let mut st = self.lock();
        let window = st.float_rate.entry(actor.person.user_id.clone()).or_default();
        if !allowed(window, now, rules::FLOATING_RATE_COUNT, rules::FLOATING_RATE_WINDOW_MS) {
            return Err(ChatError::RateLimited);
        }
        let payload = json!({ "userId": actor.person.user_id, "name": actor.person.name, "emoji": emoji, "at": now });
        self.emit(&mut st, "party.reaction", payload);
        Ok(())
    }

    pub fn set_activity(&self, status: Option<&str>) {
        let now = self.now();
        let label: Option<String> = status
            .map(|s| s.trim().chars().take(60).collect::<String>())
            .filter(|s| !s.is_empty());
        let mut st = self.lock();
        match label {
            Some(l) => {
                st.activity = Some((l.clone(), now));
                self.emit(&mut st, "party.activity", json!({ "label": l }));
            }
            None => {
                if st.activity.take().is_some() {
                    self.emit(&mut st, "party.activity", json!({ "label": Value::Null }));
                }
            }
        }
    }

    pub fn archive(&self) -> anyhow::Result<usize> {
        let days = self.host.config_i64("archiveDays", 30).clamp(1, 3650);
        let now = self.now();
        let removed = self.store.prune_messages(now - days * 86_400_000)?;
        self.store.prune_logs(now)?;
        Ok(removed)
    }
}
