use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::error::VoiceError;
use super::limits::{self, Windows};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub peer_id: String,
    pub user_id: String,
    pub name: String,
    pub device_id: String,
    pub muted: bool,
    pub watching: bool,
    pub last_beat: i64,
    pub absent_since: Option<i64>,
    pub speaking_until: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub peer_id: String,
    pub user_id: String,
    pub name: String,
    pub muted: bool,
}

impl From<&Participant> for Member {
    fn from(p: &Participant) -> Self {
        Self {
            peer_id: p.peer_id.clone(),
            user_id: p.user_id.clone(),
            name: p.name.clone(),
            muted: p.muted,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joined {
    pub peer_id: String,
    pub replaced: Option<String>,
    pub announce: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relay {
    pub to_user: String,
    pub n: u64,
}

pub struct JoinRequest<'a> {
    pub user_id: &'a str,
    pub name: &'a str,
    pub device_id: &'a str,
    pub muted: bool,
    pub watching: bool,
    pub max_peers: usize,
}

#[derive(Debug)]
pub struct VoiceRoom {
    revision: i64,
    counter: u64,
    peers: Vec<Participant>,
    pairs: BTreeMap<(String, String), u64>,
    windows: Windows,
    announced_speaking: Vec<String>,
    quiet_leaves: BTreeMap<String, i64>,
}

impl VoiceRoom {
    pub fn new(now: i64) -> Self {
        Self {
            revision: now,
            counter: 0,
            peers: Vec::new(),
            pairs: BTreeMap::new(),
            windows: Windows::default(),
            announced_speaking: Vec::new(),
            quiet_leaves: BTreeMap::new(),
        }
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }

    pub fn members(&self) -> Vec<Member> {
        self.peers.iter().map(Member::from).collect()
    }

    pub fn find(&self, peer_id: &str) -> Option<&Participant> {
        self.peers.iter().find(|p| p.peer_id == peer_id)
    }

    pub fn has_user(&self, user_id: &str) -> bool {
        self.peers.iter().any(|p| p.user_id == user_id)
    }

    pub fn join(&mut self, req: &JoinRequest, now: i64) -> Result<Joined, VoiceError> {
        if !limits::valid_device_id(req.device_id) {
            return Err(VoiceError::BadDevice);
        }
        if !self
            .windows
            .allow(format!("join:{}", req.user_id), limits::JOINS, now)
        {
            return Err(VoiceError::RateLimited);
        }
        let replaced = self
            .peers
            .iter()
            .find(|p| p.user_id == req.user_id && p.device_id == req.device_id)
            .map(|p| p.peer_id.clone());
        if replaced.is_none() && self.peers.len() >= req.max_peers {
            return Err(VoiceError::Full);
        }
        if let Some(old) = &replaced {
            self.remove(old);
        }
        self.counter += 1;
        let peer_id = format!("{now:x}-{:x}", self.counter);
        self.peers.push(Participant {
            peer_id: peer_id.clone(),
            user_id: req.user_id.into(),
            name: req.name.into(),
            device_id: req.device_id.into(),
            muted: req.muted,
            watching: req.watching,
            last_beat: now,
            absent_since: None,
            speaking_until: 0,
        });
        self.revision += 1;
        let returning = self
            .quiet_leaves
            .remove(req.user_id)
            .is_some_and(|at| now - at < limits::RETURN_GRACE_MS);
        let announce = replaced.is_none() && !returning;
        Ok(Joined {
            peer_id,
            replaced,
            announce,
        })
    }

    pub fn leave(
        &mut self,
        user_id: &str,
        peer_id: &str,
        quiet: bool,
        now: i64,
    ) -> Result<bool, VoiceError> {
        match self.find(peer_id) {
            None => Ok(false),
            Some(p) if p.user_id != user_id => Err(VoiceError::Forbidden),
            Some(_) => {
                if quiet {
                    self.quiet_leaves.insert(user_id.to_string(), now);
                }
                self.remove(peer_id);
                self.revision += 1;
                Ok(true)
            }
        }
    }

    fn remove(&mut self, peer_id: &str) -> Option<Participant> {
        let at = self.peers.iter().position(|p| p.peer_id == peer_id)?;
        self.pairs
            .retain(|(from, to), _| from != peer_id && to != peer_id);
        Some(self.peers.remove(at))
    }

    fn owned_mut(&mut self, user_id: &str, peer_id: &str) -> Result<&mut Participant, VoiceError> {
        match self.peers.iter_mut().find(|p| p.peer_id == peer_id) {
            None => Err(VoiceError::NotJoined),
            Some(p) if p.user_id != user_id => Err(VoiceError::Forbidden),
            Some(p) => Ok(p),
        }
    }

    pub fn beat(&mut self, user_id: &str, peer_id: &str, now: i64) -> Result<(), VoiceError> {
        self.owned_mut(user_id, peer_id)?.last_beat = now;
        Ok(())
    }

    pub fn set_muted(
        &mut self,
        user_id: &str,
        peer_id: &str,
        muted: bool,
        now: i64,
    ) -> Result<bool, VoiceError> {
        let peer = self.owned_mut(user_id, peer_id)?;
        peer.last_beat = now;
        if peer.muted == muted {
            return Ok(false);
        }
        peer.muted = muted;
        if muted {
            peer.speaking_until = 0;
        }
        self.revision += 1;
        Ok(true)
    }

    pub fn set_speaking(
        &mut self,
        user_id: &str,
        peer_id: &str,
        speaking: bool,
        now: i64,
    ) -> Result<(), VoiceError> {
        if speaking
            && !self
                .windows
                .allow(format!("speaking:{peer_id}"), limits::SPEAKING, now)
        {
            return Err(VoiceError::RateLimited);
        }
        let peer = self.owned_mut(user_id, peer_id)?;
        peer.last_beat = now;
        peer.speaking_until = if speaking && !peer.muted {
            now + limits::SPEAKING_TTL_MS
        } else {
            0
        };
        Ok(())
    }

    pub fn speaking(&self, now: i64) -> Vec<String> {
        self.peers
            .iter()
            .filter(|p| p.speaking_until > now)
            .map(|p| p.peer_id.clone())
            .collect()
    }

    pub fn speaking_change(&mut self, now: i64) -> Option<Vec<String>> {
        let current = self.speaking(now);
        if current == self.announced_speaking {
            return None;
        }
        self.announced_speaking.clone_from(&current);
        Some(current)
    }

    pub fn expire(
        &mut self,
        now: i64,
        present: Option<&BTreeSet<String>>,
        grace_ms: i64,
    ) -> Vec<Participant> {
        self.windows.prune(now);
        for p in &mut self.peers {
            let absent = p.watching && present.is_some_and(|set| !set.contains(&p.user_id));
            p.absent_since = if absent {
                p.absent_since.or(Some(now))
            } else {
                None
            };
        }
        let gone: Vec<String> = self
            .peers
            .iter()
            .filter(|p| {
                now - p.last_beat >= limits::TTL_MS
                    || p.absent_since.is_some_and(|at| now - at >= grace_ms)
            })
            .map(|p| p.peer_id.clone())
            .collect();
        let removed: Vec<Participant> = gone.iter().filter_map(|id| self.remove(id)).collect();
        if !removed.is_empty() {
            self.revision += 1;
        }
        removed
    }

    pub fn relay(
        &mut self,
        user_id: &str,
        from: &str,
        to: &str,
        now: i64,
    ) -> Result<Relay, VoiceError> {
        self.owned_mut(user_id, from)?;
        if from == to {
            return Err(VoiceError::SelfSignal);
        }
        if !self
            .windows
            .allow(format!("signal:{from}"), limits::SIGNALS, now)
        {
            return Err(VoiceError::RateLimited);
        }
        let to_user = self
            .find(to)
            .map(|p| p.user_id.clone())
            .ok_or(VoiceError::PeerGone)?;
        let n = self
            .pairs
            .entry((from.to_string(), to.to_string()))
            .or_insert(0);
        *n += 1;
        Ok(Relay { to_user, n: *n })
    }
}
