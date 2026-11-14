use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::model::Model;
use crate::peers::{ExternalTool, Status};
use crate::prompt::{self, OpenPoll, OpenVote};
use crate::store::{Incoming, State};
use crate::strings;
use crate::tools::{Availability, ToolSet};

mod deps;
mod events;
mod gate;
mod proactive;
mod turn;

pub use deps::Deps;
pub use turn::{Limits, Turn};

pub const MAX_AGE_MS: i64 = 180_000;
const RECONCILE_EVERY_MS: i64 = 15_000;
const HISTORY_FOR_RECONCILE: u32 = 40;
const HISTORY_IN_PROMPT: u32 = 12;
const TICK: Duration = Duration::from_millis(200);

#[derive(Default)]
struct Live {
    vote: Option<OpenVote>,
    vote_required: Option<BTreeSet<String>>,
    poll: Option<OpenPoll>,
    last_spoke: i64,
    last_reconcile: i64,
    last_offair: i64,
    names: Vec<String>,
    warned_unconfigured: bool,
}

pub struct Agent {
    pub deps: Arc<Deps>,
    live: Mutex<Live>,
    active: Mutex<Option<Box<Turn>>>,
    started: AtomicBool,
    paused: AtomicBool,
}

impl Agent {
    pub fn new(deps: Arc<Deps>) -> Arc<Self> {
        Arc::new(Self { deps, live: Mutex::new(Live { names: vec!["hermes".into(), "hermès".into()], ..Live::default() }), active: Mutex::new(None), started: AtomicBool::new(false), paused: AtomicBool::new(false) })
    }

    fn live(&self) -> std::sync::MutexGuard<'_, Live> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn start(self: &Arc<Self>) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let now = self.deps.now();
        let _ = self.deps.rules.seed_lessons(now);
        if let Ok(count) = self.deps.store.interrupt_active(now) {
            if count > 0 {
                self.deps.audit("audit", "turns_interrupted", json!({ "count": count }));
            }
        }
        let agent = self.clone();
        tokio::spawn(async move {
            while agent.started.load(Ordering::SeqCst) {
                tokio::time::sleep(TICK).await;
                let worker = agent.clone();
                let _ = kroma_module_sdk::host::run_blocking(move || worker.tick()).await;
            }
        });
    }

    pub fn stop(&self) {
        self.started.store(false, Ordering::SeqCst);
        if let Some(mut turn) = self.active.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let id = turn.message().id.clone();
            turn.abort("arret");
            let _ = self.deps.store.set_state(&id, State::Interrupted, Some("module stopped"), self.deps.now());
        }
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        self.deps.audit("audit", "paused", json!({ "paused": paused }));
    }

    pub fn is_active(&self) -> bool {
        !self.paused.load(Ordering::SeqCst) && self.deps.settings().enabled
    }

    pub fn speaking_recently(&self, now: i64) -> bool {
        let window = self.deps.settings().engage_window_s * 1000;
        now - self.live().last_spoke < window
    }

    pub fn note_spoke(&self, at: i64) {
        let mut live = self.live();
        live.last_spoke = live.last_spoke.max(at);
    }

    pub fn is_busy(&self) -> bool {
        self.active.lock().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    pub fn tick(&self) {
        if !self.is_active() {
            return;
        }
        let taken = self.active.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(mut turn) = taken else {
            self.reconcile_if_due();
            self.start_next();
            return;
        };
        match turn.advance() {
            None => *self.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(turn),
            Some(fin) => self.conclude(&turn, fin),
        }
    }

    fn conclude(&self, turn: &Turn, fin: &str) {
        let now = self.deps.now();
        if turn.answered() {
            self.note_spoke(now);
        }
        let _ = self.deps.store.set_state(&turn.message().id, State::Done, Some(fin), now);
    }

    fn tools_for(&self, model: &Arc<dyn Model>, now: i64) -> ToolSet {
        let settings = self.deps.settings();
        let availability = Availability { search: model.can_search(), vision: settings.vision && self.deps.snapshot_supported(now) };
        ToolSet::build(&self.deps, &availability, now)
    }

    fn skip(&self, message: &Incoming, reason: &str) {
        let _ = self.deps.store.set_state(&message.id, State::Skipped, Some(reason), self.deps.now());
        self.deps.audit("chat", "skipped", json!({ "message": message.id, "user": message.user_id, "reason": reason }));
    }

    fn start_next(&self) {
        let now = self.deps.now();
        let Ok(Some(row)) = self.deps.store.next_queued() else { return };
        let message = row.message;
        if now - message.at > MAX_AGE_MS * 2 {
            return self.skip(&message, "stale_in_queue");
        }
        let settings = self.deps.settings();
        let window = now - 60_000;
        if self.deps.store.engaged_since(&message.user_id, window).unwrap_or(0) >= settings.user_turns_per_minute {
            return self.skip(&message, "user_rate_limited");
        }
        if self.deps.calls_this_hour() >= settings.hourly_calls {
            self.notify_unavailable("hermes.fallback.limit", &message);
            return self.skip(&message, "hourly_limit");
        }
        let Some(model) = self.deps.model() else {
            if !std::mem::replace(&mut self.live().warned_unconfigured, true) {
                self.deps.audit("audit", "not_configured", json!({ "provider": format!("{:?}", settings.provider) }));
            }
            return self.skip(&message, "not_configured");
        };
        let tools = Arc::new(self.tools_for(&model, now));
        let prompt = self.assemble(&message, &tools, &model);
        self.deps.audit("chat", "turn_started", json!({ "message": message.id, "user": message.user_id, "tools": tools.names(), "sections": prompt.1 }));
        let dialogue = model.open(&prompt.0.text, &prompt.2, &tools.declarations());
        let _ = self.deps.store.set_state(&message.id, State::Active, None, now);
        let limits = Limits { max_steps: settings.max_steps, budget_s: settings.turn_budget_s as f64 };
        let turn = Turn::new(self.deps.clone(), model, tools, dialogue, message, limits, settings.language);
        *self.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(turn));
    }

    fn notify_unavailable(&self, key: &str, message: &Incoming) {
        let now = self.deps.now();
        if !self.deps.take_notice_slot(now) {
            return;
        }
        let text = strings::text(&self.deps.settings().language, key, &[]);
        if self.deps.room.say(&text, None, Some(&message.id)).is_ok() {
            self.note_spoke(now);
        }
    }

    fn assemble(&self, message: &Incoming, tools: &ToolSet, _model: &Arc<dyn Model>) -> (prompt::SystemPrompt, Vec<&'static str>, String) {
        let now = self.deps.now();
        let settings = self.deps.settings();
        let status: Option<Status> = self.deps.control.status().ok();
        let mut history = self.deps.room.history(HISTORY_IN_PROMPT + 1, None).unwrap_or_default();
        history.retain(|m| m.id != message.id);
        let skip = history.len().saturating_sub(HISTORY_IN_PROMPT as usize);
        history.drain(..skip);
        let notes = self.deps.store.notes(&message.user_id).unwrap_or_default();
        let (vote, poll, name) = {
            let live = self.live();
            (live.vote.clone(), live.poll.clone(), live.names.last().cloned().unwrap_or_else(|| "Hermès".into()))
        };
        let external = self.deps.contributors.tools(now).iter().any(|t: &ExternalTool| tools.has(&t.decl.name));
        let system = prompt::system_instruction(tools, &self.deps.rules.render(), &settings.language, external);
        let sections = system.sections.clone();
        let context = prompt::context(&prompt::Context {
            now,
            author: &message.name,
            text: &message.text,
            status: status.as_ref(),
            vote: vote.as_ref(),
            poll: poll.as_ref(),
            history: &history,
            notes: &notes,
            agent_name: &name,
        });
        (system, sections, context)
    }

    fn reconcile_if_due(&self) {
        let now = self.deps.now();
        {
            let mut live = self.live();
            if now - live.last_reconcile < RECONCILE_EVERY_MS {
                return;
            }
            live.last_reconcile = now;
        }
        self.reconcile();
    }

    /// Reads the room's recent history and takes in every message the events
    /// missed. A message already seen, answered or not, is never taken twice.
    pub fn reconcile(&self) -> usize {
        let Ok(messages) = self.deps.room.history(HISTORY_FOR_RECONCILE, None) else { return 0 };
        let mut recovered = 0;
        for m in messages {
            let incoming = Incoming { id: m.id, at: m.at, user_id: m.user_id.clone().unwrap_or_default(), name: m.name, text: m.text };
            if self.take(&m.kind, incoming, None) {
                recovered += 1;
            }
        }
        if recovered > 0 {
            self.deps.audit("chat", "reconciled", json!({ "recovered": recovered }));
        }
        recovered
    }

    pub fn maintain(&self) -> anyhow::Result<()> {
        let now = self.deps.now();
        let settings = self.deps.settings();
        self.reconcile();
        let audit = self.deps.store.prune_audit(now - settings.audit_days * 86_400_000)?;
        let inbox = self.deps.store.prune_inbox(now - 7 * 86_400_000)?;
        self.deps.audit("audit", "maintenance", json!({ "audit_pruned": audit, "inbox_pruned": inbox }));
        Ok(())
    }

    pub fn status_json(&self) -> Value {
        let settings = self.deps.settings();
        let now = self.deps.now();
        let live = self.live();
        json!({
            "enabled": settings.enabled,
            "paused": self.paused.load(Ordering::SeqCst),
            "provider": format!("{:?}", settings.provider).to_lowercase(),
            "model": settings.active_model(),
            "configured": settings.configured(),
            "busy": self.is_busy(),
            "speakingRecently": now - live.last_spoke < settings.engage_window_s * 1000,
            "voteOpen": live.vote.is_some(),
            "pollOpen": live.poll.is_some(),
            "callsLastHour": self.deps.calls_this_hour(),
            "hourlyLimit": settings.hourly_calls,
        })
    }
}

#[cfg(test)]
mod tests;
