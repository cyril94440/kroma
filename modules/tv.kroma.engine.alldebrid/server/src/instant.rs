//! Which of a ranked list of releases AllDebrid already holds: each is
//! uploaded, the first one it reports ready is kept, and every other magnet
//! this call created is deleted again. AllDebrid documents no signal for an
//! upload that was already in the account, so the account's magnet ids are read
//! first and an id present then is never deleted.

use std::collections::HashSet;

use anyhow::Result;

use crate::add::{classify, Link};
use crate::api::Uploaded;
use crate::AllDebrid;

pub const MAX_CANDIDATES: usize = 8;
const DELETE_PACE: std::time::Duration = std::time::Duration::from_millis(90);

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

impl AllDebrid {
    /// The best-ranked candidate that is ready at once, or `None`. An empty list
    /// answers `None` without a call, which is how a caller asks whether this
    /// engine can do it at all.
    pub fn instant(&self, candidates: &[Candidate]) -> Result<Option<Instant>> {
        let ranked = &candidates[..candidates.len().min(MAX_CANDIDATES)];
        if ranked.is_empty() {
            return Ok(None);
        }
        let before = match self.api.magnet_ids() {
            Ok(ids) => Some(ids),
            Err(error) => {
                tracing::warn!(
                    "could not list the account's magnets; none will be deleted: {error:#}"
                );
                None
            }
        };
        let mut tried = Vec::new();
        let outcome = self.try_all(ranked, &mut tried);
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
            self.remember(Some(&client_ref));
            Instant {
                key: ranked[rank].key.clone(),
                client_ref,
            }
        }))
    }

    fn try_all(&self, ranked: &[Candidate], tried: &mut Vec<Tried>) -> Result<()> {
        let mut magnets: Vec<(usize, &str)> = Vec::new();
        let mut links: Vec<(usize, &str)> = Vec::new();
        for (rank, candidate) in ranked.iter().enumerate() {
            match classify(&candidate.magnet_or_url) {
                Some(Link::Magnet(magnet)) => magnets.push((rank, magnet)),
                Some(Link::Web(url)) => links.push((rank, url)),
                None => tracing::warn!(key = %candidate.key, "not a magnet or a link; skipped"),
            }
        }
        if !magnets.is_empty() {
            let sent: Vec<&str> = magnets.iter().map(|(_, magnet)| *magnet).collect();
            for ((rank, _), answer) in magnets.iter().zip(self.api.upload_magnets(&sent)?) {
                match answer {
                    Ok(uploaded) => tried.push(Tried {
                        rank: *rank,
                        uploaded,
                    }),
                    Err(error) => tracing::warn!(rank, "AllDebrid refused a candidate: {error:#}"),
                }
            }
        }
        for (rank, url) in links {
            if tried
                .iter()
                .any(|attempt| attempt.uploaded.ready && attempt.rank < rank)
            {
                break;
            }
            match self.upload_fetched(url) {
                Ok(uploaded) => tried.push(Tried { rank, uploaded }),
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
        let mut deleted = false;
        for attempt in tried {
            let id = attempt.uploaded.id.as_str();
            if Some(id) == chosen || seen.contains(&id) {
                continue;
            }
            seen.push(id);
            if self.creds.knows(id) {
                continue;
            }
            let Some(before) = before else {
                tracing::warn!(
                    magnet = id,
                    "left in the account: its prior contents are unknown"
                );
                continue;
            };
            if before.contains(id) {
                continue;
            }
            if deleted {
                std::thread::sleep(DELETE_PACE);
            }
            deleted = true;
            if let Err(error) = self.api.delete(id) {
                tracing::warn!(
                    magnet = id,
                    "could not delete an unchosen magnet: {error:#}"
                );
            }
        }
    }
}
