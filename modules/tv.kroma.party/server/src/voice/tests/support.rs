use std::collections::BTreeSet;

use super::super::room::{JoinRequest, VoiceRoom};

pub const T0: i64 = 1_700_000_000_000;
pub const GRACE: i64 = 45_000;

pub fn room() -> VoiceRoom {
    VoiceRoom::new(T0)
}

pub fn request<'a>(user: &'a str, device: &'a str) -> JoinRequest<'a> {
    JoinRequest {
        user_id: user,
        name: user,
        device_id: device,
        muted: false,
        watching: false,
        max_peers: 8,
    }
}

pub fn join(room: &mut VoiceRoom, user: &str, device: &str) -> String {
    room.join(&request(user, device), T0).unwrap().peer_id
}

pub fn present(users: &[&str]) -> BTreeSet<String> {
    users.iter().map(|u| (*u).to_string()).collect()
}
