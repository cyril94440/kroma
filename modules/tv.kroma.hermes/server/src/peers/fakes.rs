use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use super::{Contributors, ExternalTool, KickOutcome, Room, RoomMessage, SwitchAsk, SwitchOutcome};

pub type PollCall = (String, String, Vec<String>, Option<i64>);
pub type ExternalCall = (String, String, Value, String, String);

#[derive(Debug, Clone, PartialEq)]
pub struct Said {
    pub text: String,
    pub card: Option<Value>,
    pub reply_to: Option<String>,
}

#[derive(Default)]
pub struct FakeRoom {
    pub said: Mutex<Vec<Said>>,
    pub history: Mutex<Vec<RoomMessage>>,
    pub switches: Mutex<Vec<(String, String, String)>>,
    pub switch_asks: Mutex<Vec<(Option<String>, bool)>>,
    pub go_live_reply: Mutex<Option<Result<Value, String>>>,
    pub fine_sync_reply: Mutex<Option<Result<Value, String>>>,
    pub logs_reply: Mutex<Option<Result<Vec<Value>, String>>>,
    pub logs_asks: Mutex<Vec<(Option<String>, u32, Option<u32>)>>,
    pub party_calls: Mutex<Vec<(String, String)>>,
    pub switch_reply: Mutex<Option<SwitchOutcome>>,
    pub polls: Mutex<Vec<PollCall>>,
    pub poll_error: Mutex<Option<String>>,
    pub votes: Mutex<Vec<(String, String)>>,
    pub vote_error: Mutex<Option<String>>,
    pub kicks: Mutex<Vec<(String, String, String)>>,
    pub kick_reply: Mutex<Option<KickOutcome>>,
    pub activity_log: Mutex<Vec<Option<String>>>,
    pub fail_say: Mutex<bool>,
}

impl FakeRoom {
    pub fn texts(&self) -> Vec<String> {
        self.said.lock().unwrap().iter().map(|s| s.text.clone()).collect()
    }

    pub fn push_history(&self, message: RoomMessage) {
        self.history.lock().unwrap().push(message);
    }
}

impl Room for FakeRoom {
    fn say(&self, text: &str, card: Option<Value>, reply_to: Option<&str>) -> Result<String> {
        if *self.fail_say.lock().unwrap() {
            return Err(anyhow!("room unreachable"));
        }
        let mut said = self.said.lock().unwrap();
        said.push(Said { text: text.into(), card, reply_to: reply_to.map(str::to_string) });
        Ok(format!("said-{}", said.len()))
    }

    fn history(&self, limit: u32, _before: Option<i64>) -> Result<Vec<RoomMessage>> {
        let all = self.history.lock().unwrap();
        let skip = all.len().saturating_sub(limit as usize);
        Ok(all[skip..].to_vec())
    }

    fn request_switch(&self, ask: &SwitchAsk) -> Result<SwitchOutcome> {
        self.switches.lock().unwrap().push((ask.channel_id.into(), ask.requested_by.into(), ask.reason.into()));
        self.switch_asks.lock().unwrap().push((ask.channel_name.map(str::to_string), ask.force));
        Ok(self.switch_reply.lock().unwrap().clone().unwrap_or(SwitchOutcome { outcome: "voteStarted".into(), vote_id: Some("1".into()), reason: None }))
    }

    fn start_poll(&self, question: &str, kind: &str, options: &[String], duration_s: Option<i64>) -> Result<Value> {
        if let Some(e) = self.poll_error.lock().unwrap().clone() {
            return Err(anyhow!(e));
        }
        self.polls.lock().unwrap().push((question.into(), kind.into(), options.to_vec(), duration_s));
        Ok(json!({ "question": question, "options": options }))
    }

    fn close_poll(&self) -> Result<Value> {
        match self.poll_error.lock().unwrap().clone() {
            Some(e) => Err(anyhow!(e)),
            None => Ok(json!({ "total": 2 })),
        }
    }

    fn vote_poll(&self, user_id: &str, value: &str) -> Result<()> {
        if let Some(e) = self.vote_error.lock().unwrap().clone() {
            return Err(anyhow!(e));
        }
        self.votes.lock().unwrap().push((user_id.into(), value.into()));
        Ok(())
    }

    fn kick(&self, user_id: &str, requested_by: &str, reason: &str) -> Result<KickOutcome> {
        self.kicks.lock().unwrap().push((user_id.into(), requested_by.into(), reason.into()));
        Ok(self.kick_reply.lock().unwrap().clone().unwrap_or(KickOutcome { outcome: "kicked".into(), reason: None, disruptions: 2 }))
    }

    fn activity(&self, status: Option<&str>) -> Result<()> {
        self.activity_log.lock().unwrap().push(status.map(str::to_string));
        Ok(())
    }

    fn go_live(&self, requested_by: &str) -> Result<Value> {
        self.party_calls.lock().unwrap().push(("goLive".into(), requested_by.into()));
        answer(&self.go_live_reply)
    }

    fn fine_sync(&self, requested_by: &str) -> Result<Value> {
        self.party_calls.lock().unwrap().push(("fineSync".into(), requested_by.into()));
        answer(&self.fine_sync_reply)
    }

    fn client_logs(&self, user_id: Option<&str>, limit: u32, minutes: Option<u32>) -> Result<Vec<Value>> {
        self.logs_asks.lock().unwrap().push((user_id.map(str::to_string), limit, minutes));
        answer(&self.logs_reply)
    }
}

pub fn answer<T: Clone>(slot: &Mutex<Option<Result<T, String>>>) -> Result<T> {
    match slot.lock().unwrap().clone() {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(anyhow!(e)),
        None => Err(anyhow!("HTTP 404: not found")),
    }
}

#[derive(Default)]
pub struct FakeContributors {
    pub tools: Mutex<Vec<ExternalTool>>,
    pub calls: Mutex<Vec<ExternalCall>>,
    pub answer: Mutex<Option<Result<Value, String>>>,
}

impl Contributors for FakeContributors {
    fn tools(&self, _now: i64) -> Vec<ExternalTool> {
        self.tools.lock().unwrap().clone()
    }

    fn call(&self, module_id: &str, name: &str, arguments: &Value, user_id: &str, message_id: &str) -> Result<Value> {
        self.calls.lock().unwrap().push((module_id.into(), name.into(), arguments.clone(), user_id.into(), message_id.into()));
        match self.answer.lock().unwrap().clone() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(anyhow!(e)),
            None => Ok(json!({ "ok": true })),
        }
    }
}
