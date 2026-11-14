use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use kroma_module_sdk::host::{Event, HostCtx};
use serde_json::{json, Value};

use crate::clock::{system_clock, Clock};
use crate::live::{Live, Viewer};
use crate::model::{params, text_vars, ChatMessage, Kind, Person};
use crate::poll::Poll;
use crate::store::Store;
use crate::strings;
use crate::sync::SyncGroup;
use crate::vote::Vote;

mod chat;
mod govern;
mod ideas;
mod govern_flow;
mod polls;
mod presence;
mod snapshot;
mod syncing;
mod tick;
mod telemetry;

pub use chat::ChatError;
pub use govern::{Request, SwitchResult};
pub use ideas::{IdeaError, IdeaInput};


pub const SEQ_KEY: &str = "seq.reserved";
pub const POLL_KEY: &str = "poll.state";
pub const DEPARTURE_GRACE_MS: i64 = 180_000;
pub const START_QUIET_MS: i64 = 180_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub person: Person,
    pub admin: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ViewerStats {
    pub buffer: f64,
    pub bandwidth_kbps: f64,
    pub stalls: u32,
    pub rebuffer_ms: u64,
    pub dropped: u32,
    pub resolution: String,
    pub playing: bool,
    pub delay: Option<f64>,
    pub paused: bool,
    pub at: i64,
    pub name: String,
}

#[derive(Debug, Default)]
pub(crate) struct Alert {
    pub unstable_since: Option<i64>,
    pub last_alert: i64,
}

pub(crate) struct State {
    pub seq: u64,
    pub reserved: u64,
    pub counter: u64,
    pub vote: Option<Vote>,
    pub vote_counter: u64,
    pub poll: Option<Poll>,
    pub poll_counter: u64,
    pub last_poll: Option<Value>,
    pub sync: SyncGroup,
    pub activity: Option<(String, i64)>,
    pub present: Option<Vec<Viewer>>,
    pub present_at: i64,
    pub known: BTreeMap<String, String>,
    pub leaving: BTreeMap<String, (String, i64)>,
    pub seeded: bool,
    pub rate: BTreeMap<String, Vec<i64>>,
    pub float_rate: BTreeMap<String, Vec<i64>>,
    pub stats: BTreeMap<String, ViewerStats>,
    pub alert: Alert,
    pub last_presence_poll: i64,
    pub last_sync_tick: i64,
    pub last_status_poll: i64,
    pub last_prune: i64,
    pub last_tick: i64,
}

pub struct Party {
    pub(crate) host: Arc<dyn HostCtx>,
    pub(crate) store: Store,
    pub(crate) live: Arc<dyn Live>,
    pub(crate) clock: Clock,
    pub(crate) state: Mutex<State>,
    pub(crate) started: AtomicBool,
    #[cfg(test)]
    pub(crate) tap: Mutex<Vec<(Option<String>, String, Value)>>,
}

impl Party {
    pub fn new(host: Arc<dyn HostCtx>, pool: crate::store::Pool, live: Arc<dyn Live>) -> Arc<Self> {
        Self::with_clock(host, pool, live, system_clock())
    }

    pub fn with_clock(
        host: Arc<dyn HostCtx>,
        pool: crate::store::Pool,
        live: Arc<dyn Live>,
        clock: Clock,
    ) -> Arc<Self> {
        let store = Store::new(pool);
        let now = clock();
        let reserved = store
            .kv_get(SEQ_KEY)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let seq = reserved.max(now as u64);
        let party = Arc::new(Self {
            host,
            store,
            live,
            clock,
            state: Mutex::new(State {
                seq,
                reserved: seq,
                counter: 0,
                vote: None,
                vote_counter: 0,
                poll: None,
                poll_counter: 0,
                last_poll: None,
                sync: SyncGroup::new(now + START_QUIET_MS),
                activity: None,
                present: None,
                present_at: 0,
                known: BTreeMap::new(),
                leaving: BTreeMap::new(),
                seeded: false,
                rate: BTreeMap::new(),
                float_rate: BTreeMap::new(),
                stats: BTreeMap::new(),
                alert: Alert::default(),
                last_presence_poll: 0,
                last_sync_tick: 0,
                last_status_poll: 0,
                last_prune: 0,
                last_tick: now,
            }),
            started: AtomicBool::new(false),
            #[cfg(test)]
            tap: Mutex::new(Vec::new()),
        });
        party.restore_poll();
        party
    }

    pub(crate) fn now(&self) -> i64 {
        (self.clock)()
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn vote_seconds(&self) -> i64 {
        self.host.config_i64("voteSeconds", 30).clamp(5, 600)
    }

    pub(crate) fn grace_ms(&self) -> i64 {
        self.host.config_i64("presenceGraceSeconds", 45).clamp(0, 600) * 1000
    }

    pub fn agent_name(&self) -> String {
        let name = self.host.config_str("agentName", "Hermès");
        if name.trim().is_empty() { "Hermès".into() } else { name }
    }

    pub(crate) fn may_bypass(&self, actor: &Actor) -> bool {
        actor.admin
            || self
                .host
                .config_str("bypassUserIds", "")
                .split(',')
                .any(|id| !id.trim().is_empty() && id.trim() == actor.person.user_id)
    }

    pub(crate) fn emit(&self, st: &mut State, topic: &str, mut payload: Value) -> u64 {
        st.seq += 1;
        if st.seq >= st.reserved {
            st.reserved = st.seq + 1000;
            let _ = self.store.kv_set(SEQ_KEY, &st.reserved.to_string());
        }
        payload["seq"] = json!(st.seq);
        #[cfg(test)]
        self.tap.lock().unwrap().push((None, topic.to_string(), payload.clone()));
        self.host.publish(Event::new(topic, payload));
        st.seq
    }

    pub(crate) fn emit_to(&self, user_id: &str, topic: &str, payload: Value) {
        #[cfg(test)]
        self.tap.lock().unwrap().push((Some(user_id.to_string()), topic.to_string(), payload.clone()));
        self.host.publish_to(user_id, Event::new(topic, payload));
    }

    pub(crate) fn next_id(&self, st: &mut State, at: i64) -> String {
        st.counter += 1;
        format!("{at:x}-{:x}", st.counter)
    }

    pub(crate) fn record_message(&self, st: &mut State, mut m: ChatMessage) -> ChatMessage {
        if m.id.is_empty() {
            m.id = self.next_id(st, m.at);
        }
        if let Err(e) = self.store.insert_message(&m) {
            tracing::error!(error = %e, "chat message not stored");
        }
        let payload = chat::message_event(&m, &self.agent_name());
        self.emit(st, "party.message", payload);
        m
    }

    pub(crate) fn system_message(&self, st: &mut State, key: &str, pairs: &[(&str, &str)], action: Option<&str>) -> ChatMessage {
        let map = params(pairs);
        self.system_message_with(st, key, map, action)
    }

    pub(crate) fn system_message_with(
        &self,
        st: &mut State,
        key: &str,
        map: serde_json::Map<String, Value>,
        action: Option<&str>,
    ) -> ChatMessage {
        let text = strings::text("en", key, &text_vars(&map));
        let m = ChatMessage {
            kind: Kind::System,
            name: String::new(),
            text,
            key: Some(key.into()),
            params: map,
            action: action.map(str::to_string),
            at: self.now(),
            ..ChatMessage::default()
        };
        self.record_message(st, m)
    }

    pub(crate) fn fetch_presence(&self) -> Option<Vec<Viewer>> {
        let viewers = self.live.presence().ok()?;
        let mut seen = std::collections::BTreeSet::new();
        Some(
            viewers
                .into_iter()
                .filter(|v| seen.insert(v.user_id.clone()))
                .collect(),
        )
    }
}

pub(crate) fn people(viewers: &[Viewer]) -> Vec<Person> {
    viewers.iter().map(|v| Person::new(&v.user_id, &v.name)).collect()
}

#[cfg(test)]
mod tests;
