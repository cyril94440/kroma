use serde_json::json;

use super::{people, Actor, Party};
use crate::model::Person;
use crate::vote::{Opening, Recorded, Vote, VoteKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchResult {
    pub outcome: &'static str,
    pub vote_id: Option<String>,
    pub reason: Option<String>,
}

impl SwitchResult {
    fn refused(reason: &str) -> Self {
        Self {
            outcome: "refused",
            vote_id: None,
            reason: Some(reason.into()),
        }
    }
}

pub struct Request<'a> {
    pub channel_id: &'a str,
    pub channel_name: Option<&'a str>,
    pub requester: Option<Person>,
    pub note: &'a str,
    pub force: bool,
}

impl Party {
    fn channel_label(&self, channel_id: &str, hint: Option<&str>) -> String {
        let found = self
            .live
            .resolve(channel_id, 5)
            .ok()
            .and_then(|list| list.into_iter().find(|c| c.id == channel_id))
            .map(|c| c.name);
        found
            .or_else(|| hint.map(str::to_string))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| channel_id.to_string())
    }

    pub fn find_channels(&self, query: &str) -> Vec<crate::live::Candidate> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        self.live.resolve(query, 8).unwrap_or_default()
    }

    pub fn request_switch(&self, req: Request) -> SwitchResult {
        let requester_id = req.requester.as_ref().map_or("agent", |p| p.user_id.as_str()).to_string();
        let label = self.channel_label(req.channel_id, req.channel_name);
        if let Ok(status) = self.live.status() {
            if status.channel.as_ref().is_some_and(|c| c.id == req.channel_id) {
                return SwitchResult::refused("alreadyOn");
            }
        }
        if req.force {
            return self.switch_now(req.channel_id, &label, req.requester.as_ref(), &requester_id, "forced", "party.vote.forced");
        }
        if let Some(open) = self.lock().vote.as_ref() {
            return SwitchResult {
                outcome: "refused",
                vote_id: Some(open.id.to_string()),
                reason: Some("busy".into()),
            };
        }
        let Some(viewers) = self.fetch_presence() else {
            self.trace("vote_presence_unavailable", json!({ "requester": requester_id, "channel": label }));
            return SwitchResult::refused("presenceUnavailable");
        };
        let present = people(&viewers);
        if Vote::has_quorum(&present, req.requester.as_ref()) {
            let now = self.now();
            let mut st = self.lock();
            if let Some(open) = st.vote.as_ref() {
                return SwitchResult {
                    outcome: "refused",
                    vote_id: Some(open.id.to_string()),
                    reason: Some("busy".into()),
                };
            }
            st.vote_counter += 1;
            let kind = VoteKind::Channel {
                channel_id: req.channel_id.into(),
                channel_name: label.clone(),
            };
            let vote = Vote::open(Opening {
                id: st.vote_counter,
                kind,
                label: &label,
                note: req.note,
                requester: req.requester.clone(),
                present: &present,
                now,
                seconds: self.vote_seconds(),
            });
            let id = vote.id;
            let name = req.requester.as_ref().map(|p| p.name.clone());
            st.vote = Some(vote);
            self.announce_opened(&mut st, &label, name.as_deref(), req.note);
            self.publish_vote(&mut st);
            return SwitchResult {
                outcome: "voteStarted",
                vote_id: Some(id.to_string()),
                reason: None,
            };
        }
        self.switch_now(req.channel_id, &label, req.requester.as_ref(), &requester_id, "alone", "party.vote.solo")
    }

    fn switch_now(
        &self,
        channel_id: &str,
        label: &str,
        requester: Option<&Person>,
        requester_id: &str,
        reason: &str,
        key: &str,
    ) -> SwitchResult {
        match self.live.switch(channel_id, requester_id, reason) {
            Ok(done) if done.switched => {
                let name = requester.map_or_else(|| self.agent_name(), |p| p.name.clone());
                let mut st = self.lock();
                self.system_message(&mut st, key, &[("channel", label), ("name", &name)], Some("channel_vote_end"));
                SwitchResult {
                    outcome: "switchedDirectly",
                    vote_id: None,
                    reason: None,
                }
            }
            Ok(done) => SwitchResult::refused(done.error.as_deref().unwrap_or("offAir")),
            Err(e) => SwitchResult::refused(&format!("{e:#}")),
        }
    }

    pub fn cast_vote(&self, voter: &Person, approve: bool) -> Result<(), &'static str> {
        let needs_presence = {
            let st = self.lock();
            let Some(vote) = st.vote.as_ref() else {
                return Err("noVote");
            };
            if !vote.accepts_voter(&voter.user_id) {
                return Err("requester");
            }
            !vote.is_required(&voter.user_id)
        };
        let present = if needs_presence { self.fetch_presence().map(|v| people(&v)) } else { None };
        let decided = {
            let mut st = self.lock();
            let Some(vote) = st.vote.as_mut() else {
                return Err("noVote");
            };
            let id = vote.id;
            let result = vote.record(voter, approve, present.as_deref());
            if result == Recorded::Ignored {
                return Err("notPresent");
            }
            self.trace("vote_cast", json!({ "user": voter.user_id, "approve": approve, "vote": id }));
            if result == Recorded::Decided {
                st.vote.take()
            } else {
                self.publish_vote(&mut st);
                None
            }
        };
        if let Some(vote) = decided {
            self.finalize(vote);
        }
        Ok(())
    }

    pub fn actor_vote(&self, actor: &Actor, approve: bool) -> Result<(), &'static str> {
        self.cast_vote(&actor.person, approve)
    }
}
