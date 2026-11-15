use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use serde_json::Value;

use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

use crate::live::{Candidate, ChannelRef, Live, Status, Switched, Viewer};
use crate::model::{ChatMessage, Person};
use crate::party::Party;

pub const T0: i64 = 1_700_000_000_000;

#[derive(Default)]
pub struct FakeLive {
    pub present: Mutex<Option<Vec<Viewer>>>,
    pub status: Mutex<Status>,
    pub switches: Mutex<Vec<(String, String, String)>>,
    pub proposals: Mutex<Vec<(String, bool)>>,
    pub switch_error: Mutex<Option<String>>,
    pub names: Mutex<BTreeMap<String, String>>,
}

impl Live for FakeLive {
    fn status(&self) -> Result<Status> {
        Ok(self.status.lock().unwrap().clone())
    }

    fn presence(&self) -> Result<Vec<Viewer>> {
        self.present.lock().unwrap().clone().ok_or_else(|| anyhow!("live unreachable"))
    }

    fn switch(&self, channel_id: &str, requested_by: &str, reason: &str) -> Result<Switched> {
        self.switches
            .lock()
            .unwrap()
            .push((channel_id.into(), requested_by.into(), reason.into()));
        if let Some(error) = self.switch_error.lock().unwrap().clone() {
            return Ok(Switched { switched: false, channel: None, error: Some(error) });
        }
        self.status.lock().unwrap().channel = Some(ChannelRef { id: channel_id.into(), name: channel_id.into() });
        Ok(Switched { switched: true, channel: None, error: None })
    }

    fn proposal(&self, id: &str, accept: bool) -> Result<bool> {
        self.proposals.lock().unwrap().push((id.into(), accept));
        Ok(true)
    }

    fn resolve(&self, query: &str, _limit: u32) -> Result<Vec<Candidate>> {
        Ok(self
            .names
            .lock()
            .unwrap()
            .get(query)
            .map(|name| vec![Candidate { id: query.into(), name: name.clone(), ..Candidate::default() }])
            .unwrap_or_default())
    }
}

pub struct Rig {
    pub party: Arc<Party>,
    pub live: Arc<FakeLive>,
    pub host: Arc<StubHost>,
    now: Arc<AtomicI64>,
}

pub fn rig() -> Rig {
    rig_with(StubHost::with_db("party"))
}

pub fn rig_no_grace() -> Rig {
    rig_with(StubHost::with_db("party").with_config("presenceGraceSeconds", serde_json::json!(0)))
}

pub fn rig_with(host: StubHost) -> Rig {
    rig_with_turn(host, Arc::new(crate::voice::CloudflareTurn))
}

pub fn rig_with_turn(host: StubHost, turn: Arc<dyn crate::voice::TurnSource>) -> Rig {
    let pool = host.store().clone();
    pool.get().unwrap().execute_batch(crate::store::MIGRATIONS).unwrap();
    let host = Arc::new(host);
    let live = Arc::new(FakeLive::default());
    let now = Arc::new(AtomicI64::new(T0));
    let clock_now = now.clone();
    let ctx: Arc<dyn HostCtx> = host.clone();
    let party = Party::with_turn(ctx, pool, live.clone(), Arc::new(move || clock_now.load(Ordering::SeqCst)), turn);
    Rig { party, live, host, now }
}

impl Rig {
    pub fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn present(&self, names: &[&str]) {
        *self.live.present.lock().unwrap() = Some(
            names
                .iter()
                .map(|n| Viewer { user_id: n.to_lowercase(), name: (*n).into(), since: T0 })
                .collect(),
        );
    }

    pub fn unreachable(&self) {
        *self.live.present.lock().unwrap() = None;
    }

    pub fn texts(&self) -> Vec<String> {
        self.party.history(500, None).into_iter().map(|m| m.text).collect()
    }

    pub fn keys(&self) -> Vec<String> {
        self.party.history(500, None).into_iter().filter_map(|m| m.key).collect()
    }

    pub fn last_system(&self) -> ChatMessage {
        self.party
            .history(500, None)
            .into_iter()
            .rev()
            .find(|m| m.key.is_some())
            .expect("a system message")
    }

    pub fn switched_to(&self) -> Vec<String> {
        self.live.switches.lock().unwrap().iter().map(|s| s.0.clone()).collect()
    }

    pub fn events(&self, topic: &str) -> Vec<Value> {
        self.party
            .tap
            .lock()
            .unwrap()
            .iter()
            .filter(|(to, t, _)| to.is_none() && t == topic)
            .map(|(_, _, p)| p.clone())
            .collect()
    }

    pub fn all_events(&self) -> Vec<(Option<String>, String, Value)> {
        self.party.tap.lock().unwrap().clone()
    }

    pub fn vote_open(&self) -> bool {
        self.party.lock().vote.is_some()
    }

    pub fn vote_required(&self) -> Vec<String> {
        self.party.lock().vote.as_ref().map(|v| v.required.keys().cloned().collect()).unwrap_or_default()
    }

    pub fn ask(&self, who: &str, channel: &str) -> crate::party::SwitchResult {
        self.party.request_switch(crate::party::Request {
            channel_id: channel,
            channel_name: None,
            requester: Some(person(who)),
            note: "",
            force: false,
        })
    }
}

pub fn person(name: &str) -> Person {
    Person::new(&name.to_lowercase(), name)
}
