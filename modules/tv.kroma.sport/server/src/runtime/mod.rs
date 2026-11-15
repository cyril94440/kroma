use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use kroma_module_sdk::host::{Event, HostCtx};
use serde_json::{json, Value};

use crate::clock::{system_clock, Clock};
use crate::espn::Espn;
use crate::peers::{Live, Room};
use crate::prono::{File, Pronos};
use crate::settings::Settings;
use crate::store::Store;

mod announce;
mod board;
mod calibration;
mod clips;
mod detect;
mod follow;
mod guide;
mod lookup;
mod prono;
mod roles;
mod scan;
mod snapshot;
mod standings;
mod state;
mod status;
mod sweep;
mod tick;
mod trace;

pub use calibration::CalibrationError;
pub use prono::VoteError;
pub use state::State;

pub struct Sport {
    pub(crate) host: Arc<dyn HostCtx>,
    pub(crate) store: Store,
    pub(crate) espn: Arc<dyn Espn>,
    pub(crate) live: Arc<dyn Live>,
    pub(crate) room: Arc<dyn Room>,
    pub(crate) clock: Clock,
    pub(crate) state: Mutex<State>,
    pub(crate) started: AtomicBool,
    #[cfg(test)]
    pub(crate) tap: Mutex<Vec<(String, Value)>>,
}

impl Sport {
    pub fn new(
        host: Arc<dyn HostCtx>,
        store: Store,
        espn: Arc<dyn Espn>,
        live: Arc<dyn Live>,
        room: Arc<dyn Room>,
    ) -> Arc<Self> {
        Self::with_clock(host, store, espn, live, room, system_clock())
    }

    pub fn with_clock(
        host: Arc<dyn HostCtx>,
        store: Store,
        espn: Arc<dyn Espn>,
        live: Arc<dyn Live>,
        room: Arc<dyn Room>,
        clock: Clock,
    ) -> Arc<Self> {
        let now = clock();
        let reserved = store
            .kv_get(state::SEQ_KEY)
            .ok()
            .flatten()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let seq = reserved.max(now as u64);
        let settings = Settings::read(&*host);
        let pronos_file: File = store.kv_json(state::PRONO_KEY).unwrap_or_default();
        let follow_saved: state::FollowSaved = store.kv_json(state::FOLLOW_KEY).unwrap_or_default();
        let announced: BTreeSet<String> = store.kv_json(state::ANNOUNCED_KEY).unwrap_or_default();
        let mut inner = State {
            seq,
            reserved: seq,
            presence: Default::default(),
            events: Default::default(),
            guide: state::Guide {
                links: store.kv_json(state::LINKS_KEY).unwrap_or_default(),
                ..Default::default()
            },
            memory: store.kv_json(state::MEMORY_KEY).unwrap_or_default(),
            scan: Default::default(),
            board: Default::default(),
            detect: Default::default(),
            follow: state::Follow {
                saved: follow_saved,
                ..Default::default()
            },
            pronos: Pronos::from_file(
                pronos_file,
                settings.spoiler_delay_s,
                settings.spoiler_margin_s,
            ),
            lag: Default::default(),
            prono_at: 0.0,
            standings: Default::default(),
            clips: Default::default(),
            announced,
            views: Default::default(),
            admins: Default::default(),
        };
        inner.clips.done = store
            .clips(None, 500)
            .map(|rows| {
                rows.into_iter()
                    .map(|c| (c.id, c.ts as f64 / 1000.0))
                    .collect()
            })
            .unwrap_or_default();
        inner.clips.attempts = store.attempts().unwrap_or_default();
        Arc::new(Self {
            host,
            store,
            espn,
            live,
            room,
            clock,
            state: Mutex::new(inner),
            started: AtomicBool::new(false),
            #[cfg(test)]
            tap: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    pub(crate) fn now(&self) -> f64 {
        self.now_ms() as f64 / 1000.0
    }

    pub(crate) fn settings(&self) -> Settings {
        Settings::read(&*self.host)
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn emit(&self, st: &mut State, topic: &str, mut payload: Value) -> u64 {
        st.seq += 1;
        if st.seq >= st.reserved {
            st.reserved = st.seq + 1000;
            self.store.kv_put(state::SEQ_KEY, &st.reserved.to_string());
        }
        payload["seq"] = json!(st.seq);
        #[cfg(test)]
        self.tap
            .lock()
            .unwrap()
            .push((topic.to_string(), payload.clone()));
        self.host.publish(Event::new(topic, payload));
        st.seq
    }

    pub(crate) fn emit_if_changed(
        &self,
        st: &mut State,
        topic: &'static str,
        payload: Value,
        fingerprint: String,
    ) {
        if st.views.get(topic) == Some(&fingerprint) {
            return;
        }
        st.views.insert(topic, fingerprint);
        self.emit(st, topic, payload);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
pub(crate) mod tests;
