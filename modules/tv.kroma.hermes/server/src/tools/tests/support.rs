use std::sync::Arc;

use serde_json::Value;

use crate::model::Model;
use crate::store::Incoming;
use crate::testing::{Rig, Scripted};
use crate::tools::{Availability, Backend, ToolEnv, ToolSet, TurnCtx};

pub fn run(rig: &Rig, model: &Scripted, name: &str, args: Value, user: &str) -> (Value, TurnCtx) {
    let model: Arc<dyn Model> = Arc::new(model.clone());
    let message: Incoming = rig.incoming("msg-1", user, "texte");
    let tools = ToolSet::build(&rig.deps, &Availability { search: true, vision: true }, rig.now());
    let tool = tools.get(name).unwrap_or_else(|| panic!("tool {name} is not declared"));
    let mut ctx = TurnCtx::default();
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
