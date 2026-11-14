use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};
use serde_json::{json, Value};

use crate::agent::{Agent, Deps};
use crate::model::{Allowed, Dialogue, Model, ModelError, ModelSource, ScreenReading, ToolCall, ToolDecl, WebAnswer};
use crate::peers::fakes::{FakeContributors, FakeControl, FakeRoom};
use crate::peers::{Candidate, ChannelRef, Programme, RoomMessage, Slot, Status, Viewer};
use crate::store::{Incoming, Store};

pub const START: i64 = 1_794_000_000_000;

pub enum Step {
    Calls(Vec<(&'static str, Value)>),
    Raw(Vec<ToolCall>),
    Fail(ModelError),
    Nothing,
}

#[derive(Default)]
pub struct Script {
    steps: Mutex<VecDeque<Step>>,
    pub allowed: Mutex<Vec<Allowed>>,
    pub answers: Mutex<Vec<Vec<(ToolCall, Value)>>>,
    pub opened: Mutex<Vec<(String, String, Vec<String>)>>,
    pub search: Mutex<Option<Result<WebAnswer, ModelError>>>,
    pub searches: Mutex<Vec<String>>,
    pub screen: Mutex<Option<Result<ScreenReading, ModelError>>>,
    pub per_step: Mutex<Option<(Arc<AtomicI64>, i64)>>,
    pub no_search: Mutex<bool>,
}

#[derive(Clone)]
pub struct Scripted(pub Arc<Script>);

impl Scripted {
    pub fn new(steps: Vec<Step>) -> Self {
        let script = Script::default();
        *script.steps.lock().unwrap() = steps.into();
        Self(Arc::new(script))
    }

    pub fn remaining(&self) -> usize {
        self.0.steps.lock().unwrap().len()
    }

}

struct ScriptedDialogue(Arc<Script>);

impl Dialogue for ScriptedDialogue {
    fn step(&mut self, allowed: &Allowed) -> Result<Vec<ToolCall>, ModelError> {
        self.0.allowed.lock().unwrap().push(allowed.clone());
        if let Some((clock, ms)) = self.0.per_step.lock().unwrap().as_ref() {
            clock.fetch_add(*ms, Ordering::SeqCst);
        }
        match self.0.steps.lock().unwrap().pop_front() {
            Some(Step::Calls(calls)) => Ok(calls.into_iter().enumerate().map(|(i, (name, args))| ToolCall { id: format!("c{i}"), name: name.into(), args, malformed: None }).collect()),
            Some(Step::Raw(calls)) => Ok(calls),
            Some(Step::Fail(e)) => Err(e),
            Some(Step::Nothing) | None => Ok(Vec::new()),
        }
    }

    fn answer(&mut self, results: &[(ToolCall, Value)]) {
        self.0.answers.lock().unwrap().push(results.to_vec());
    }
}

impl Model for Scripted {
    fn open(&self, system: &str, first: &str, tools: &[ToolDecl]) -> Box<dyn Dialogue> {
        self.0.opened.lock().unwrap().push((system.into(), first.into(), tools.iter().map(|t| t.name.clone()).collect()));
        Box::new(ScriptedDialogue(self.0.clone()))
    }

    fn models(&self) -> Result<Vec<String>, ModelError> {
        Ok(vec!["scripted".into()])
    }

    fn ping(&self) -> Result<String, ModelError> {
        Ok("ok".into())
    }

    fn web_search(&self, question: &str) -> Result<WebAnswer, ModelError> {
        self.0.searches.lock().unwrap().push(question.into());
        self.0.search.lock().unwrap().clone().unwrap_or(Err(ModelError::Rejected("no search scripted".into())))
    }

    fn read_screen(&self, _jpeg_base64: &str, _question: &str) -> Result<ScreenReading, ModelError> {
        self.0.screen.lock().unwrap().clone().unwrap_or(Err(ModelError::Rejected("no screen scripted".into())))
    }

    fn can_search(&self) -> bool {
        !*self.0.no_search.lock().unwrap()
    }
}

pub struct Rig {
    pub host: StubHost,
    pub room: Arc<FakeRoom>,
    pub control: Arc<FakeControl>,
    pub contributors: Arc<FakeContributors>,
    pub clock: Arc<AtomicI64>,
    pub agent: Arc<Agent>,
    pub deps: Arc<Deps>,
    pub model: Arc<Mutex<Option<Arc<dyn Model>>>>,
}

pub fn candidate(id: &str, name: &str, score: f64) -> Candidate {
    Candidate { id: id.into(), name: name.into(), group: "FR".into(), score, now: None }
}

pub fn programme(channel_id: &str, channel_name: &str, title: &str, start_min: i64, end_min: i64) -> Programme {
    Programme { channel_id: channel_id.into(), channel_name: channel_name.into(), title: title.into(), description: String::new(), start: START + start_min * 60_000, end: START + end_min * 60_000 }
}

impl Rig {
    pub fn new() -> Self {
        Self::with_config(&[])
    }

    pub fn with_config(config: &[(&str, Value)]) -> Self {
        let mut host = StubHost::with_db("hermes-rig");
        for (key, value) in config {
            host = host.with_config(key, value.clone());
        }
        host.store().get().unwrap().execute_batch(crate::store::MIGRATIONS).unwrap();
        let room = Arc::new(FakeRoom::default());
        let control = Arc::new(FakeControl::default());
        *control.status.lock().unwrap() = Status {
            state: "running".into(),
            channel: Some(ChannelRef { id: "tf1".into(), name: "TF1".into() }),
            provider: "primary".into(),
            now: Some(Slot { title: "Journal".into(), start: START - 600_000, end: START + 600_000 }),
            viewers: vec![Viewer { user_id: "u-mamie".into(), name: "Mamie".into() }, Viewer { user_id: "u-papi".into(), name: "Papi".into() }],
            ..Status::default()
        };
        *control.channels.lock().unwrap() = vec![candidate("tf1", "TF1", 10.0), candidate("m6", "M6", 9.0), candidate("arte", "Arte", 8.0)];
        let contributors = Arc::new(FakeContributors::default());
        let clock = Arc::new(AtomicI64::new(START));
        let model: Arc<Mutex<Option<Arc<dyn Model>>>> = Arc::new(Mutex::new(None));
        let source = model.clone();
        let settings_host = host.clone();
        let models: ModelSource = Arc::new(move || source.lock().unwrap().clone().or_else(|| crate::model::model_from(&crate::settings::Settings::read(&settings_host))));
        let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
        let ticking = clock.clone();
        let deps = Arc::new(Deps::new(ctx, room.clone(), control.clone(), contributors.clone(), Store::new(host.store().clone()), Arc::new(move || ticking.load(Ordering::SeqCst)), models));
        let agent = Agent::new(deps.clone());
        Self { host, room, control, contributors, clock, agent, deps, model }
    }

    pub fn use_model(&self, model: Arc<dyn Model>) {
        *self.model.lock().unwrap() = Some(model);
    }

    pub fn now(&self) -> i64 {
        self.clock.load(Ordering::SeqCst)
    }

    pub fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
    }

    pub fn incoming(&self, id: &str, user: &str, text: &str) -> Incoming {
        Incoming { id: id.into(), at: self.now(), user_id: format!("u-{}", user.to_lowercase()), name: user.into(), text: text.into() }
    }

    pub fn chat(&self, id: &str, user: &str, text: &str) -> Value {
        let m = self.incoming(id, user, text);
        json!({ "id": m.id, "userId": m.user_id, "name": m.name, "text": m.text, "at": m.at, "kind": "user" })
    }

    pub fn hear(&self, id: &str, user: &str, text: &str) {
        self.agent.on_event("party.message", &self.chat(id, user, text));
    }

    pub fn history_user(&self, id: &str, user: &str, text: &str) {
        self.room.push_history(RoomMessage { id: id.into(), user_id: Some(format!("u-{}", user.to_lowercase())), name: user.into(), kind: "user".into(), text: text.into(), at: self.now() });
    }

    pub fn drain(&self) -> usize {
        for ticks in 0..400 {
            self.agent.tick();
            if !self.agent.is_busy() && self.deps.store.next_queued().unwrap().is_none() {
                return ticks;
            }
        }
        panic!("the agent never went idle");
    }

    pub fn turns(&self) -> Vec<Value> {
        self.deps.store.audit_rows(Some("audit"), None, 200).unwrap().into_iter().filter(|r| r.event == "agent_turn").map(|r| r.data).rev().collect()
    }

}
