use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::gate::{decide, Room, Signals, Verdict};
use super::{Agent, MAX_AGE_MS};
use crate::prompt::{OpenPoll, OpenVote};
use crate::store::{Incoming, State};

fn text_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

impl Agent {
    pub(super) fn take(&self, kind: &str, message: Incoming, event: Option<&Value>) -> bool {
        match kind {
            "agent" => {
                self.note_spoke(message.at);
                if !message.name.trim().is_empty() {
                    let name = message.name.trim().to_lowercase();
                    let mut live = self.live();
                    if !live.names.contains(&name) {
                        live.names.push(name);
                    }
                }
                return false;
            }
            "user" => {}
            _ => return false,
        }
        if message.user_id.is_empty() || message.id.is_empty() || message.text.trim().is_empty() || !self.is_active() {
            return false;
        }
        let now = self.deps.now();
        if now - message.at > MAX_AGE_MS {
            self.deps.store.claim(&message, State::Skipped, Some("stale"), now).ok();
            return false;
        }
        let (signals, room) = {
            let live = self.live();
            let signals = Signals::of(&message.text, &live.names, event);
            let room = Room { vote_required: live.vote_required.clone(), poll_open: live.poll.is_some(), spoke_recently: false };
            (signals, room)
        };
        let room = Room { spoke_recently: self.speaking_recently(now), ..room };
        let verdict = decide(&message.text, &signals, &message.user_id, &room);
        let (state, reason) = match verdict {
            Verdict::Engage => (State::Queued, None),
            Verdict::Skip(reason) => (State::Skipped, Some(reason)),
        };
        let claimed = self.deps.store.claim(&message, state, reason, now).unwrap_or(false);
        if claimed {
            self.deps.audit(
                "chat",
                "gate",
                json!({
                    "message": message.id, "user": message.user_id, "text": message.text, "verdict": format!("{verdict:?}"),
                    "mentions": signals.mentions_agent, "question": signals.question, "keyword": signals.keyword,
                    "spoke_recently": room.spoke_recently, "poll_open": room.poll_open, "vote_answer_expected": room.vote_required.is_some(),
                }),
            );
        }
        claimed && verdict == Verdict::Engage
    }

    pub fn on_event(&self, topic: &str, payload: &Value) {
        match topic {
            "party.message" => {
                let message = Incoming {
                    id: text_of(payload, "id").into(),
                    at: payload.get("at").and_then(Value::as_i64).unwrap_or_else(|| self.deps.now()),
                    user_id: text_of(payload, "userId").into(),
                    name: text_of(payload, "name").into(),
                    text: text_of(payload, "text").into(),
                };
                self.take(text_of(payload, "kind"), message, Some(payload));
            }
            "party.vote" => self.on_vote(payload),
            "party.poll" => self.on_poll(payload),
            "live.status" => self.on_live_status(payload),
            "live.failover" => self.deps.audit("sources", "live_failover", payload.clone()),
            _ => {}
        }
    }

    fn on_vote(&self, payload: &Value) {
        let mut live = self.live();
        let Some(vote) = payload.get("vote").filter(|v| v.is_object()) else {
            live.vote = None;
            live.vote_required = None;
            return;
        };
        let required: Vec<(String, String)> = vote
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|p| (text_of(p, "userId").to_string(), text_of(p, "name").to_string()))
            .collect();
        let voted: BTreeSet<&str> = vote.get("votes").and_then(Value::as_object).map(|m| m.keys().map(String::as_str).collect()).unwrap_or_default();
        live.vote_required = Some(required.iter().map(|(id, _)| id.clone()).collect());
        live.vote = Some(OpenVote {
            label: text_of(vote, "label").into(),
            requester: vote.pointer("/by/name").and_then(Value::as_str).unwrap_or("?").into(),
            waiting_on: required.iter().filter(|(id, _)| !voted.contains(id.as_str())).map(|(_, name)| name.clone()).collect(),
        });
    }

    fn on_poll(&self, payload: &Value) {
        let mut live = self.live();
        live.poll = payload.get("poll").filter(|p| p.is_object()).map(|p| OpenPoll {
            question: text_of(p, "question").into(),
            kind: text_of(p, "kind").into(),
            options: p.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
        });
    }
}
