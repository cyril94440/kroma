use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::Person;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteKind {
    Channel { channel_id: String, channel_name: String },
    Provider { proposal_id: String, proposal_kind: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub yes: u32,
    pub no: u32,
    pub blank: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Passed,
    Refused,
    Alone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reevaluated {
    Unchanged,
    Updated,
    Decided,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Ignored,
    Counted,
    Decided,
}

#[derive(Debug, Clone)]
pub struct Vote {
    pub id: u64,
    pub kind: VoteKind,
    pub label: String,
    pub note: String,
    pub requester: Option<Person>,
    pub required: BTreeMap<String, String>,
    pub votes: BTreeMap<String, bool>,
    pub opened_at: i64,
    pub deadline: i64,
    absent_since: BTreeMap<String, i64>,
    empty_reads: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoteView {
    pub id: u64,
    pub kind: &'static str,
    pub label: String,
    pub note: String,
    pub by: Option<Person>,
    pub ends_at: i64,
    pub ends_in_s: i64,
    pub required: Vec<Person>,
    pub votes: BTreeMap<String, bool>,
    pub tally: Tally,
    pub channel_id: Option<String>,
}

pub struct Opening<'a> {
    pub id: u64,
    pub kind: VoteKind,
    pub label: &'a str,
    pub note: &'a str,
    pub requester: Option<Person>,
    pub present: &'a [Person],
    pub now: i64,
    pub seconds: i64,
}

impl Vote {
    pub fn open(o: Opening) -> Self {
        let Opening { id, kind, label, note, requester, present, now, seconds } = o;
        let required = present
            .iter()
            .filter(|p| requester.as_ref().is_none_or(|r| r.user_id != p.user_id))
            .map(|p| (p.user_id.clone(), p.name.clone()))
            .collect();
        Self {
            id,
            kind,
            label: label.into(),
            note: note.into(),
            requester,
            required,
            votes: BTreeMap::new(),
            opened_at: now,
            deadline: now + seconds * 1000,
            absent_since: BTreeMap::new(),
            empty_reads: 0,
        }
    }

    pub fn has_quorum(present: &[Person], requester: Option<&Person>) -> bool {
        present
            .iter()
            .any(|p| requester.is_none_or(|r| r.user_id != p.user_id))
    }

    pub fn tally(&self) -> Tally {
        let counted = u32::from(self.requester.is_some());
        let mut yes = counted;
        let mut no = 0;
        let mut blank = 0;
        for id in self.required.keys() {
            match self.votes.get(id) {
                Some(true) => yes += 1,
                Some(false) => no += 1,
                None => blank += 1,
            }
        }
        Tally { yes, no, blank }
    }

    fn decided_ahead(&self) -> bool {
        let t = self.tally();
        t.yes > t.no + t.blank || t.yes + t.blank <= t.no
    }

    fn all_voted(&self) -> bool {
        self.required.keys().all(|id| self.votes.contains_key(id))
    }

    pub fn is_requester(&self, user_id: &str) -> bool {
        self.requester.as_ref().is_some_and(|r| r.user_id == user_id)
    }

    pub fn is_required(&self, user_id: &str) -> bool {
        self.required.contains_key(user_id)
    }

    pub fn accepts_voter(&self, user_id: &str) -> bool {
        !self.is_requester(user_id)
    }

    pub fn record(&mut self, voter: &Person, approve: bool, present: Option<&[Person]>) -> Recorded {
        if self.is_requester(&voter.user_id) {
            return Recorded::Ignored;
        }
        if !self.required.contains_key(&voter.user_id) {
            let here = present.is_some_and(|p| p.iter().any(|x| x.user_id == voter.user_id));
            if !here {
                return Recorded::Ignored;
            }
            self.required.insert(voter.user_id.clone(), voter.name.clone());
        }
        self.votes.insert(voter.user_id.clone(), approve);
        if self.all_voted() || self.decided_ahead() {
            Recorded::Decided
        } else {
            Recorded::Counted
        }
    }

    pub fn reevaluate(&mut self, present: &[Person], now: i64, grace_ms: i64) -> Reevaluated {
        let here = |id: &str| present.iter().any(|p| p.user_id == id);
        for id in self.required.keys() {
            if here(id) {
                self.absent_since.remove(id);
            } else {
                self.absent_since.entry(id.clone()).or_insert(now);
            }
        }
        let remaining: BTreeMap<String, String> = self
            .required
            .iter()
            .filter(|(id, _)| here(id) || now - self.absent_since[*id] < grace_ms)
            .map(|(id, name)| (id.clone(), name.clone()))
            .collect();
        if !self.required.is_empty() && remaining.is_empty() {
            self.empty_reads += 1;
            if self.empty_reads < 2 {
                return Reevaluated::Unchanged;
            }
        } else {
            self.empty_reads = 0;
        }
        let changed = remaining.len() != self.required.len();
        self.required = remaining;
        let requester_here = self.requester.as_ref().is_some_and(|r| here(&r.user_id));
        if self.required.is_empty() && !requester_here {
            Reevaluated::Cancelled
        } else if self.required.is_empty() || self.all_voted() || self.decided_ahead() {
            Reevaluated::Decided
        } else if changed {
            Reevaluated::Updated
        } else {
            Reevaluated::Unchanged
        }
    }

    pub fn verdict(&self) -> Verdict {
        let t = self.tally();
        if self.required.is_empty() {
            Verdict::Alone
        } else if t.yes > t.no {
            Verdict::Passed
        } else {
            Verdict::Refused
        }
    }

    pub fn view(&self, now: i64) -> VoteView {
        let (kind, channel_id) = match &self.kind {
            VoteKind::Channel { channel_id, .. } => ("channel", Some(channel_id.clone())),
            VoteKind::Provider { .. } => ("provider", None),
        };
        VoteView {
            id: self.id,
            kind,
            label: self.label.clone(),
            note: self.note.clone(),
            by: self.requester.clone(),
            ends_at: self.deadline,
            ends_in_s: ((self.deadline - now) / 1000).max(0),
            required: self
                .required
                .iter()
                .map(|(id, name)| Person::new(id, name))
                .collect(),
            votes: self.votes.clone(),
            tally: self.tally(),
            channel_id,
        }
    }
}

#[cfg(test)]
mod tests;
