use serde_json::{json, Value};

use super::{people, Party, State};
use crate::vote::{Reevaluated, Verdict, Vote, VoteKind};

impl Party {
    pub(super) fn trace(&self, event: &str, data: Value) {
        tracing::info!(event, data = %data, "party");
    }

    pub(super) fn announce_opened(&self, st: &mut State, label: &str, by: Option<&str>, note: &str) {
        let seconds = self.vote_seconds().to_string();
        let name = by.map(str::to_string).unwrap_or_else(|| self.agent_name());
        let key = if by.is_some() { "party.vote.opened" } else { "party.vote.openedByAgent" };
        self.system_message(
            st,
            key,
            &[("name", &name), ("channel", label), ("seconds", &seconds), ("note", note)],
            None,
        );
    }

    pub(super) fn publish_vote(&self, st: &mut State) {
        let now = self.now();
        let view = st.vote.as_ref().map(|v| v.view(now));
        self.emit(st, "party.vote", json!({ "vote": view, "closed": Value::Null }));
    }

    pub(super) fn publish_vote_closed(&self, st: &mut State, vote: &Vote, verdict: &str) {
        let t = vote.tally();
        self.emit(
            st,
            "party.vote",
            json!({ "vote": Value::Null,
                    "closed": { "id": vote.id, "verdict": verdict, "yes": t.yes, "no": t.no, "blank": t.blank } }),
        );
    }

    pub(super) fn remember(&self, vote: &Vote, verdict: &str) {
        let t = vote.tally();
        let kind = match vote.kind {
            VoteKind::Channel { .. } => "channel",
            VoteKind::Provider { .. } => "provider",
        };
        let record = crate::store::VoteRecord {
            vote_id: vote.id,
            kind,
            label: &vote.label,
            requester_id: vote.requester.as_ref().map(|r| r.user_id.as_str()),
            requester_name: vote.requester.as_ref().map(|r| r.name.as_str()),
            yes: t.yes,
            no: t.no,
            blank: t.blank,
            verdict,
            opened_at: vote.opened_at,
            closed_at: self.now(),
        };
        if let Err(e) = self.store.record_vote(&record) {
            tracing::error!(error = %e, "vote history not stored");
        }
    }

    pub(super) fn finalize(&self, vote: Vote) {
        let verdict = vote.verdict();
        let t = vote.tally();
        let counts = [("yes", t.yes.to_string()), ("no", t.no.to_string())];
        let applied = match &vote.kind {
            VoteKind::Channel { channel_id, channel_name } => {
                self.finalize_channel(&vote, verdict, channel_id, channel_name, &counts)
            }
            VoteKind::Provider { proposal_id, proposal_kind } => {
                self.finalize_provider(verdict, proposal_id, proposal_kind)
            }
        };
        let label = match (verdict, applied) {
            (Verdict::Refused, _) => "refused",
            (_, true) => "passed",
            (_, false) => "impossible",
        };
        self.trace("vote_closed", json!({ "vote": vote.id, "verdict": label, "yes": t.yes, "no": t.no, "blank": t.blank }));
        self.remember(&vote, label);
        let mut st = self.lock();
        self.publish_vote_closed(&mut st, &vote, label);
    }

    fn finalize_channel(&self, vote: &Vote, verdict: Verdict, id: &str, name: &str, counts: &[(&str, String); 2]) -> bool {
        let who = vote.requester.as_ref().map_or("agent", |p| p.user_id.as_str());
        if verdict == Verdict::Refused {
            let mut st = self.lock();
            self.system_message(
                &mut st,
                "party.vote.refused",
                &[("channel", name), ("yes", &counts[0].1), ("no", &counts[1].1)],
                Some("channel_vote_end"),
            );
            return false;
        }
        match self.live.switch(id, who, "vote") {
            Ok(done) if done.switched => {
                let mut st = self.lock();
                let key = if verdict == Verdict::Alone { "party.vote.alone" } else { "party.vote.passed" };
                self.system_message(
                    &mut st,
                    key,
                    &[("channel", name), ("yes", &counts[0].1), ("no", &counts[1].1)],
                    Some("channel_vote_end"),
                );
                true
            }
            other => {
                let error = match other {
                    Ok(done) => done.error.unwrap_or_else(|| "offAir".into()),
                    Err(e) => format!("{e:#}"),
                };
                let mut st = self.lock();
                self.system_message(&mut st, "party.vote.switchFailed", &[("channel", name), ("error", &error)], Some("channel_vote_end"));
                false
            }
        }
    }

    fn finalize_provider(&self, verdict: Verdict, proposal_id: &str, kind: &str) -> bool {
        let accept = verdict != Verdict::Refused;
        let applied = self.live.proposal(proposal_id, accept).unwrap_or(false);
        let key = match (accept, applied) {
            (true, true) => "party.vote.providerAccepted",
            (true, false) => "party.vote.providerFailed",
            (false, _) => "party.vote.providerDeclined",
        };
        let mut st = self.lock();
        self.system_message(&mut st, key, &[("kind", kind)], Some("channel_vote_end"));
        accept && applied
    }

    pub(super) fn reevaluate_vote(&self) {
        if self.lock().vote.is_none() {
            return;
        }
        let Some(viewers) = self.fetch_presence() else {
            return;
        };
        let present = people(&viewers);
        let now = self.now();
        let grace = self.grace_ms();
        let mut st = self.lock();
        let Some(vote) = st.vote.as_mut() else {
            return;
        };
        match vote.reevaluate(&present, now, grace) {
            Reevaluated::Unchanged => {}
            Reevaluated::Updated => self.publish_vote(&mut st),
            Reevaluated::Decided => {
                let vote = st.vote.take();
                drop(st);
                if let Some(v) = vote {
                    self.finalize(v);
                }
            }
            Reevaluated::Cancelled => {
                let vote = st.vote.take();
                if let Some(v) = vote {
                    self.trace("vote_cancelled_absent", json!({ "vote": v.id }));
                    self.remember(&v, "cancelled");
                    self.publish_vote_closed(&mut st, &v, "cancelled");
                }
            }
        }
    }

    pub(super) fn expire_vote(&self) {
        let now = self.now();
        let due = {
            let mut st = self.lock();
            if st.vote.as_ref().is_some_and(|v| v.deadline <= now) {
                st.vote.take()
            } else {
                None
            }
        };
        if let Some(v) = due {
            self.finalize(v);
        }
    }
}
