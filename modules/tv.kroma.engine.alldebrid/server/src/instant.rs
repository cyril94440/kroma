//! Which of a ranked list of releases AllDebrid already holds: magnets are
//! uploaded a few at a time in rank order and the walk stops at the first batch
//! holding a ready one; `.torrent` links are tried one by one, only when no
//! magnet was ready. Every magnet this call created and did not keep is deleted
//! again. AllDebrid documents no signal for an upload that was already in the
//! account, so the account's ids are read first and only an id above all of
//! them is ever deleted.

use std::collections::HashSet;
use std::time::{Duration, Instant as Clock};

use anyhow::Result;

use crate::add::{classify, Link};
use crate::api::{is_retry_later, Refused, Uploaded};
use crate::AllDebrid;

pub const MAX_CANDIDATES: usize = 8;
pub const BATCH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub key: String,
    pub magnet_or_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instant {
    pub key: String,
    pub client_ref: String,
}

struct Tried {
    rank: usize,
    uploaded: Uploaded,
}

struct Deadline(Clock);

impl Deadline {
    fn after(budget: Duration) -> Self {
        Self(Clock::now() + budget)
    }

    fn left(&self) -> Duration {
        self.0.saturating_duration_since(Clock::now())
    }

    fn passed(&self) -> bool {
        self.left().is_zero()
    }
}

impl AllDebrid {
    /// The best-ranked candidate that is ready at once, or `None`, within
    /// `budget` in all. An empty list answers `None` without a call, which is
    /// how a caller asks whether this engine can do it at all.
    pub fn instant(&self, candidates: &[Candidate], budget: Duration) -> Result<Option<Instant>> {
        let ranked = &candidates[..candidates.len().min(MAX_CANDIDATES)];
        if ranked.is_empty() {
            return Ok(None);
        }
        let deadline = Deadline::after(budget);
        let before = self.snapshot();
        let mut tried = Vec::new();
        let outcome = self.try_all(ranked, &deadline, &mut tried);
        let chosen = tried
            .iter()
            .filter(|attempt| attempt.uploaded.ready)
            .min_by_key(|attempt| attempt.rank)
            .map(|attempt| (attempt.rank, attempt.uploaded.id.clone()));
        self.discard_others(
            &tried,
            chosen.as_ref().map(|(_, id)| id.as_str()),
            before.as_ref(),
        );
        outcome?;
        Ok(chosen.map(|(rank, client_ref)| {
            self.claim_if_new(before.as_ref(), &client_ref);
            self.remember(Some(&client_ref));
            Instant {
                key: ranked[rank].key.clone(),
                client_ref,
            }
        }))
    }

    fn try_all(&self, ranked: &[Candidate], deadline: &Deadline, tried: &mut Vec<Tried>) -> Result<()> {
        let mut magnets: Vec<(usize, &str)> = Vec::new();
        let mut links: Vec<(usize, &str)> = Vec::new();
        for (rank, candidate) in ranked.iter().enumerate() {
            match classify(&candidate.magnet_or_url) {
                Some(Link::Magnet(magnet)) => magnets.push((rank, magnet)),
                Some(Link::Web(url)) => links.push((rank, url)),
                None => tracing::warn!(key = %candidate.key, "not a magnet or a link; skipped"),
            }
        }
        for batch in magnets.chunks(BATCH) {
            if deadline.passed() {
                return Ok(());
            }
            let api = self.api.within(deadline.left());
            let sent: Vec<&str> = batch.iter().map(|(_, magnet)| *magnet).collect();
            let mut full = false;
            for ((rank, _), answer) in batch.iter().zip(api.upload_magnets(&sent)?) {
                match answer {
                    Ok(uploaded) => tried.push(Tried {
                        rank: *rank,
                        uploaded,
                    }),
                    Err(error) => {
                        full |= error
                            .downcast_ref::<Refused>()
                            .is_some_and(Refused::is_capacity);
                        tracing::warn!(rank, "AllDebrid refused a candidate: {error:#}");
                    }
                }
            }
            if full || tried.iter().any(|attempt| attempt.uploaded.ready) {
                return Ok(());
            }
        }
        for (rank, url) in links {
            if deadline.passed() {
                return Ok(());
            }
            match self.upload_fetched(url, deadline.left()) {
                Ok(uploaded) => {
                    let ready = uploaded.ready;
                    tried.push(Tried { rank, uploaded });
                    if ready {
                        return Ok(());
                    }
                }
                Err(error) if is_capacity(&error) || is_retry_later(&error) => return Err(error),
                Err(error) => {
                    tracing::warn!(rank, "a candidate's torrent file did not upload: {error:#}")
                }
            }
        }
        Ok(())
    }

    fn discard_others(
        &self,
        tried: &[Tried],
        chosen: Option<&str>,
        before: Option<&HashSet<String>>,
    ) {
        let mut seen: Vec<&str> = Vec::new();
        for attempt in tried {
            let id = attempt.uploaded.id.as_str();
            if Some(id) == chosen || seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if !crate::owned::is_new(before, id) {
                continue;
            }
            match self.api.delete(id) {
                Ok(()) => {}
                Err(error) if is_retry_later(&error) => {
                    tracing::warn!(magnet = id, "AllDebrid is rate limiting; remaining unchosen magnets are left to expire");
                    return;
                }
                Err(error) => {
                    tracing::warn!(magnet = id, "could not delete an unchosen magnet: {error:#}");
                }
            }
        }
    }
}

fn is_capacity(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<Refused>()
        .is_some_and(Refused::is_capacity)
}
