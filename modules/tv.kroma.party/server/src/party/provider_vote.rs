use serde_json::{json, Value};

use super::{people, Party};
use crate::live::ProposalEvent;
use crate::vote::{Opening, Vote, VoteKind};

const VOTED_KINDS: [&str; 3] = ["switchToBackup", "returnToPrimary", "switchProvider"];

impl Party {
    pub fn on_proposal(&self, payload: &Value) {
        let Ok(event) = serde_json::from_value::<ProposalEvent>(payload.clone()) else {
            return;
        };
        if event.resolved {
            self.close_provider_vote(&event);
        } else {
            self.open_provider_vote(&event);
        }
    }

    fn open_provider_vote(&self, event: &ProposalEvent) {
        let id = event.proposal.id.clone();
        let kind = event.proposal.kind.clone();
        if id.is_empty() || !VOTED_KINDS.contains(&kind.as_str()) {
            return;
        }
        if let Some(open) = self.lock().vote.as_ref() {
            if !matches!(&open.kind, VoteKind::Provider { proposal_id, .. } if *proposal_id == id) {
                let _ = self.live.proposal(&id, false);
            }
            return;
        }
        let viewers = self.fetch_presence().unwrap_or_default();
        let present = people(&viewers);
        let now = self.now();
        let mut st = self.lock();
        if st.vote.is_some() {
            drop(st);
            let _ = self.live.proposal(&id, false);
            return;
        }
        st.vote_counter += 1;
        let vote = Vote::open(Opening {
            id: st.vote_counter,
            kind: VoteKind::Provider { proposal_id: id, proposal_kind: kind.clone() },
            label: &kind,
            note: "",
            requester: None,
            present: &present,
            now,
            seconds: self.vote_seconds(),
        });
        let alone = vote.required.is_empty();
        st.vote = Some(vote);
        if alone {
            let vote = st.vote.take();
            drop(st);
            if let Some(v) = vote {
                self.finalize(v);
            }
            return;
        }
        let key = match kind.as_str() {
            "switchToBackup" => "party.vote.providerOpened.switchToBackup",
            "switchProvider" => "party.vote.providerOpened.switchProvider",
            _ => "party.vote.providerOpened.returnToPrimary",
        };
        let seconds = self.vote_seconds().to_string();
        self.system_message(&mut st, key, &[("seconds", &seconds)], None);
        self.publish_vote(&mut st);
    }

    fn close_provider_vote(&self, event: &ProposalEvent) {
        let mut st = self.lock();
        let open = st.vote.as_ref().is_some_and(
            |v| matches!(&v.kind, VoteKind::Provider { proposal_id, .. } if *proposal_id == event.proposal.id),
        );
        let Some(vote) = st.vote.take_if(|_| open) else {
            return;
        };
        self.trace(
            "vote_closed_by_live",
            json!({ "vote": vote.id, "accepted": event.accepted, "default": event.default }),
        );
        self.remember(&vote, "cancelled");
        self.system_message(&mut st, "party.vote.providerExpired", &[], Some("channel_vote_end"));
        self.publish_vote_closed(&mut st, &vote, "cancelled");
    }
}
