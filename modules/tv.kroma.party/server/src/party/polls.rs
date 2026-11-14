use serde_json::{json, Value};

use super::{people, Party, State, POLL_KEY};
use crate::model::Person;
use crate::poll::{Poll, PollKind, PollView};

impl Party {
    fn persist_poll(&self, st: &State) {
        let body = json!({ "active": st.poll, "last": st.last_poll, "counter": st.poll_counter });
        if let Err(e) = self.store.kv_set(POLL_KEY, &body.to_string()) {
            tracing::error!(error = %e, "poll state not stored");
        }
    }

    pub(super) fn restore_poll(&self) {
        let Some(saved) = self
            .store
            .kv_get(POLL_KEY)
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        else {
            return;
        };
        let now = self.now();
        let mut st = self.lock();
        st.poll_counter = saved["counter"].as_u64().unwrap_or(0);
        st.last_poll = saved.get("last").filter(|v| !v.is_null()).cloned();
        let Some(poll) = saved
            .get("active")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value::<Poll>(v.clone()).ok())
        else {
            return;
        };
        st.poll_counter = st.poll_counter.max(poll.id);
        if poll.ends_at > now {
            st.poll = Some(poll);
        } else {
            let mut view = poll.view(poll.ends_at);
            view.closed_because = Some("restarted".into());
            st.last_poll = serde_json::to_value(&view).ok();
            self.persist_poll(&st);
        }
    }

    pub fn start_poll(&self, question: &str, kind: &str, options: &[String], seconds: Option<i64>) -> Result<PollView, String> {
        let now = self.now();
        let mut st = self.lock();
        if st.poll.is_some() {
            return Err("pollAlreadyActive".into());
        }
        let poll = Poll::start(st.poll_counter + 1, question, kind, options, seconds, now).map_err(|e| e.code().to_string())?;
        st.poll_counter = poll.id;
        let view = poll.view(now);
        let key = match poll.kind {
            PollKind::Yesno | PollKind::Choice => "party.poll.started",
            PollKind::Score => "party.poll.startedScore",
            PollKind::Open => "party.poll.startedOpen",
        };
        let secs = ((poll.ends_at - now) / 1000).to_string();
        let q = poll.question.clone();
        st.poll = Some(poll);
        self.persist_poll(&st);
        self.system_message(&mut st, key, &[("question", &q), ("seconds", &secs)], None);
        self.emit(&mut st, "party.poll", json!({ "poll": view, "closed": Value::Null }));
        Ok(view)
    }

    fn close_poll_locked(&self, st: &mut State, reason: &str) -> Option<PollView> {
        let poll = st.poll.take()?;
        let now = self.now();
        let mut view = poll.view(now);
        view.closed_because = Some(reason.into());
        st.last_poll = serde_json::to_value(&view).ok();
        self.persist_poll(st);
        let summary = Poll::summary(&view);
        let results = serde_json::to_string(&view.by_answer).unwrap_or_default();
        let kind = serde_json::to_value(view.kind).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        if let Err(e) = self.store.record_poll(view.id, &view.question, &kind, &results, reason, now) {
            tracing::error!(error = %e, "poll history not stored");
        }
        let key = format!("party.poll.closed.{reason}");
        self.system_message(st, &key, &[("question", &view.question), ("summary", &summary)], None);
        self.emit(st, "party.poll", json!({ "poll": Value::Null, "closed": view }));
        Some(view)
    }

    pub fn close_poll(&self, reason: &str) -> Result<PollView, &'static str> {
        let mut st = self.lock();
        self.close_poll_locked(&mut st, reason).ok_or("noActivePoll")
    }

    pub fn vote_poll(&self, who: &Person, raw: &str) -> Result<(), &'static str> {
        let poll_id = {
            let now = self.now();
            let mut st = self.lock();
            let Some(poll) = st.poll.as_mut() else {
                return Err("noActivePoll");
            };
            if !poll.vote(who, raw) {
                return Err("invalidValue");
            }
            let id = poll.id;
            let view = poll.view(now);
            self.persist_poll(&st);
            self.emit(&mut st, "party.poll", json!({ "poll": view, "closed": Value::Null }));
            id
        };
        if let Some(viewers) = self.fetch_presence() {
            let present = people(&viewers);
            let mut st = self.lock();
            if st.poll.as_ref().is_some_and(|p| p.id == poll_id && p.everyone_voted(&present)) {
                self.close_poll_locked(&mut st, "everyoneVoted");
            }
        }
        Ok(())
    }

    pub(super) fn poll_from_chat(&self, who: &Person, text: &str) {
        let typed = self.lock().poll.as_ref().and_then(|p| p.answer_from_chat(text));
        if let Some(raw) = typed {
            let _ = self.vote_poll(who, &raw);
        }
    }

    pub(super) fn expire_poll(&self) {
        let now = self.now();
        let mut st = self.lock();
        if st.poll.as_ref().is_some_and(|p| p.ends_at <= now) {
            self.close_poll_locked(&mut st, "timeUp");
        }
    }
}
