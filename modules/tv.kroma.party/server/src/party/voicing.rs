use std::collections::BTreeSet;

use kroma_module_sdk::host::Event;
use serde_json::{json, Value};

use super::{Actor, Party};
use crate::voice::{ice, JoinRequest, Signal, VoiceError, VoiceRoom};

const BEAT_SECONDS: i64 = 10;
const EXPIRE_SECONDS: i64 = 30;

fn roster(room: &VoiceRoom) -> Value {
    json!({ "revision": room.revision(), "participants": room.members() })
}

fn speaking(room: &VoiceRoom, now: i64) -> Value {
    json!({ "speaking": room.speaking(now), "at": now })
}

impl Party {
    fn voice_room(&self) -> std::sync::MutexGuard<'_, VoiceRoom> {
        self.voice.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn voice_emit(&self, topic: &str, payload: Value) {
        #[cfg(test)]
        self.tap
            .lock()
            .unwrap()
            .push((None, topic.to_string(), payload.clone()));
        self.host.publish(Event::new(topic, payload));
    }

    fn voice_announce(&self, roster_changed: bool) {
        let now = self.now();
        let (roster_event, speaking_event) = {
            let mut room = self.voice_room();
            let roster_event = roster_changed.then(|| roster(&room));
            let speaking_event = room.speaking_change(now).map(|_| speaking(&room, now));
            (roster_event, speaking_event)
        };
        if let Some(payload) = roster_event {
            self.voice_emit("party.voice.roster", payload);
        }
        if let Some(payload) = speaking_event {
            self.voice_emit("party.voice.speaking", payload);
        }
    }

    fn voice_max_peers(&self) -> usize {
        self.host.config_i64("voiceMaxPeers", 8).clamp(2, 16) as usize
    }

    pub fn voice_state(&self) -> Value {
        let now = self.now();
        let room = self.voice_room();
        json!({
            "revision": room.revision(),
            "now": now,
            "participants": room.members(),
            "speaking": room.speaking(now),
            "max": self.voice_max_peers(),
        })
    }

    pub fn voice_join(
        &self,
        who: &Actor,
        device_id: &str,
        muted: bool,
        watching: bool,
    ) -> Result<Value, VoiceError> {
        let now = self.now();
        let joined = self.voice_room().join(
            &JoinRequest {
                user_id: &who.person.user_id,
                name: &who.person.name,
                device_id,
                muted,
                watching,
                max_peers: self.voice_max_peers(),
            },
            now,
        )?;
        tracing::info!(replaced = joined.replaced.is_some(), "voice join");
        if joined.announce {
            self.say_voice("party.voice.joined", &who.person.name);
        }
        self.voice_announce(true);
        let servers = self.voice_ice_servers();
        let room = self.voice_room();
        Ok(json!({
            "peerId": joined.peer_id,
            "revision": room.revision(),
            "participants": room.members(),
            "speaking": room.speaking(now),
            "iceServers": servers,
            "beatSeconds": BEAT_SECONDS,
            "expireSeconds": EXPIRE_SECONDS,
        }))
    }

    fn voice_ice_servers(&self) -> Vec<ice::IceServer> {
        ice::ice_servers(&*self.host, &*self.turn, &self.ice, self.now())
    }

    pub fn voice_ice(&self, user_id: &str) -> Result<Value, VoiceError> {
        if !self.voice_room().has_user(user_id) {
            return Err(VoiceError::NotJoined);
        }
        Ok(json!({ "iceServers": self.voice_ice_servers() }))
    }

    pub fn voice_leave(&self, user_id: &str, peer_id: &str, quiet: bool) -> Result<(), VoiceError> {
        let name = self.voice_room().find(peer_id).map(|p| p.name.clone());
        let now = self.now();
        let left = self.voice_room().leave(user_id, peer_id, quiet, now)?;
        if let (true, Some(name)) = (left, name) {
            tracing::info!("voice leave");
            if !quiet {
                self.say_voice("party.voice.left", &name);
            }
            self.voice_announce(true);
        }
        Ok(())
    }

    fn say_voice(&self, key: &str, name: &str) {
        let mut st = self.lock();
        self.system_message(&mut st, key, &[("name", name)], None);
    }

    pub fn voice_beat(&self, user_id: &str, peer_id: &str) -> Result<i64, VoiceError> {
        let now = self.now();
        let mut room = self.voice_room();
        room.beat(user_id, peer_id, now)?;
        Ok(room.revision())
    }

    pub fn voice_mute(&self, user_id: &str, peer_id: &str, muted: bool) -> Result<(), VoiceError> {
        let now = self.now();
        let changed = self.voice_room().set_muted(user_id, peer_id, muted, now)?;
        self.voice_announce(changed);
        Ok(())
    }

    pub fn voice_speaking(&self, user_id: &str, peer_id: &str, on: bool) -> Result<(), VoiceError> {
        let now = self.now();
        self.voice_room().set_speaking(user_id, peer_id, on, now)?;
        self.voice_announce(false);
        Ok(())
    }

    pub fn voice_signal(&self, user_id: &str, signal: &Signal) -> Result<u64, VoiceError> {
        signal.validate()?;
        let now = self.now();
        let relay = self
            .voice_room()
            .relay(user_id, &signal.from, &signal.to, now)?;
        self.emit_to(
            &relay.to_user,
            "party.voice.signal",
            signal.payload(user_id, relay.n),
        );
        Ok(relay.n)
    }

    pub(super) fn voice_tick(&self) {
        let now = self.now();
        let present: Option<BTreeSet<String>> = self
            .lock()
            .present
            .as_ref()
            .map(|viewers| viewers.iter().map(|v| v.user_id.clone()).collect());
        let removed = self
            .voice_room()
            .expire(now, present.as_ref(), self.grace_ms());
        if !removed.is_empty() {
            tracing::info!(count = removed.len(), "voice participants expired");
        }
        self.voice_announce(!removed.is_empty());
    }
}
