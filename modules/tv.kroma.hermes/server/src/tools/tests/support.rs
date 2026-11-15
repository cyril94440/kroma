use std::sync::Arc;

use serde_json::Value;

use crate::model::Model;
use crate::store::Incoming;
use crate::testing::{Rig, Scripted};
use crate::tools::{Availability, Backend, ToolEnv, ToolSet, TurnCtx};

pub fn run(rig: &Rig, model: &Scripted, name: &str, args: Value, user: &str) -> (Value, TurnCtx) {
    run_left(rig, model, name, args, user, None)
}

pub fn run_left(rig: &Rig, model: &Scripted, name: &str, args: Value, user: &str, left_s: Option<f64>) -> (Value, TurnCtx) {
    let model: Arc<dyn Model> = Arc::new(model.clone());
    let message: Incoming = rig.incoming("msg-1", user, "texte");
    let tools = ToolSet::build(&rig.deps, &Availability { search: true, vision: true, ..Availability::default() }, rig.now());
    let tool = tools.get(name).unwrap_or_else(|| panic!("tool {name} is not declared"));
    let mut ctx = TurnCtx { left_s, ..TurnCtx::default() };
    let mut env = ToolEnv { deps: &rig.deps, ctx: &mut ctx, model: &model, message: &message };
    let out = match &tool.backend {
        Backend::Builtin(handler) => handler(&mut env, &args),
        Backend::External(_) => panic!("builtin expected"),
    };
    (out, ctx)
}

pub fn call(rig: &Rig, name: &str, args: Value, user: &str) -> Value {
    run(rig, &Scripted::new(Vec::new()), name, args, user).0
}

pub fn point(index: i64, kbps: f64) -> crate::peers::HistoryPoint {
    crate::peers::HistoryPoint { at: 1_793_300_400 + 10 * index, kbps, channel: Some("TF1".into()), provider: Some("primary".into()), health: Some("healthy".into()), restarts: 3, ..Default::default() }
}

pub fn history(points: Vec<crate::peers::HistoryPoint>) -> crate::peers::History {
    crate::peers::History { step_seconds: 10, points, provider_events: Vec::new() }
}

pub fn reply<T>(slot: &crate::peers::fake_control::Reply<T>, value: T) {
    *slot.lock().unwrap() = Some(Ok(value));
}

pub fn fail<T>(slot: &crate::peers::fake_control::Reply<T>, error: &str) {
    *slot.lock().unwrap() = Some(Err(error.into()));
}
