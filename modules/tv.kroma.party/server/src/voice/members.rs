use serde::Serialize;

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
