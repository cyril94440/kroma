use std::sync::Arc;

use serde_json::{json, Value};

use crate::agent::Deps;
use crate::model::{Model, ToolDecl};
use crate::store::Incoming;

mod cards;
mod chart;
mod chat;
mod external;
mod guide;
mod history;
mod live;
mod memory;
mod ops;
mod party;
mod probe;
mod provider;
mod scan;
mod vision;
mod watch;
mod web;

pub use cards::validate_card;
pub use memory::LESSONS_MAX;
pub use scan::{Follow, ScanJob};

const LONG_TOOL_MIN_S: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Action,
    Investigation,
    Reply,
    Ignore,
    Refusable,
    Effect,
}

impl Kind {
    pub fn is_final(self) -> bool {
        matches!(self, Kind::Reply | Kind::Ignore)
    }

    pub fn is_refusable(self) -> bool {
        matches!(self, Kind::Action | Kind::Refusable)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Availability {
    pub search: bool,
    pub vision: bool,
    pub off: Vec<&'static str>,
}

impl Availability {
    pub fn on(&self, method: &str) -> bool {
        !self.off.contains(&method)
    }
}

pub struct ToolEnv<'a> {
    pub deps: &'a Deps,
    pub ctx: &'a mut TurnCtx,
    pub model: &'a Arc<dyn Model>,
    pub message: &'a Incoming,
}

#[derive(Debug, Default)]
pub struct TurnCtx {
    pub answered: bool,
    pub diffusion_done: bool,
    pub partial_seen: bool,
    pub reply_is_progress: bool,
    pub progress_posted: bool,
    pub progress_any: bool,
    pub scans: u32,
    pub left_s: Option<f64>,
    pub pending: Option<scan::ScanJob>,
}

pub type Handler = fn(&mut ToolEnv, &Value) -> Value;

pub enum Backend {
    Builtin(Handler),
    External(String),
}

pub struct ToolDef {
    pub name: String,
    pub kind: Kind,
    pub activity: Option<&'static str>,
    pub decl: ToolDecl,
    pub backend: Backend,
}

impl ToolDef {
    pub fn kind_for(&self, args: &Value) -> Kind {
        if self.name == "run_ops" && ops::is_restart(args) {
            Kind::Refusable
        } else {
            self.kind
        }
    }
}

pub struct Spec {
    pub name: &'static str,
    pub kind: Kind,
    pub activity: Option<&'static str>,
    pub available: fn(&Availability) -> bool,
    pub describe: fn() -> (&'static str, Value),
    pub handler: Handler,
}

pub fn always(_: &Availability) -> bool {
    true
}

pub struct ToolSet {
    tools: Vec<ToolDef>,
}

impl ToolSet {
    pub fn build(deps: &Deps, availability: &Availability, now: i64) -> Self {
        let mut tools: Vec<ToolDef> = specs()
            .into_iter()
            .filter(|s| (s.available)(availability))
            .map(|s| {
                let (description, parameters) = (s.describe)();
                ToolDef {
                    name: s.name.into(),
                    kind: s.kind,
                    activity: s.activity,
                    decl: ToolDecl { name: s.name.into(), description: description.into(), parameters },
                    backend: Backend::Builtin(s.handler),
                }
            })
            .collect();
        for external in deps.contributors.tools(now) {
            if tools.iter().any(|t| t.name == external.decl.name) {
                continue;
            }
            tools.push(ToolDef {
                name: external.decl.name.clone(),
                kind: Kind::Investigation,
                activity: Some("working"),
                decl: external.decl,
                backend: Backend::External(external.module_id),
            });
        }
        Self { tools }
    }

    pub fn get(&self, name: &str) -> Option<&ToolDef> {
        self.tools.iter().find(|t| t.name == name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.name.as_str()).collect()
    }

    pub fn declarations(&self) -> Vec<ToolDecl> {
        self.tools.iter().map(|t| t.decl.clone()).collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn final_names(&self) -> Vec<String> {
        self.tools.iter().filter(|t| t.kind.is_final()).map(|t| t.name.clone()).collect()
    }
}

fn specs() -> Vec<Spec> {
    let mut all = Vec::new();
    all.extend(chat::specs());
    all.extend(live::specs());
    all.extend(guide::specs());
    all.extend(party::specs());
    all.extend(watch::specs());
    all.extend(probe::specs());
    all.extend(provider::specs());
    all.extend(scan::specs());
    all.extend(history::specs());
    all.extend(memory::specs());
    all.extend(web::specs());
    all.extend(vision::specs());
    all
}

pub fn text_arg(args: &Value, key: &str) -> String {
    args.get(key).and_then(Value::as_str).unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn int_arg(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))).unwrap_or(default)
}

pub fn failure(reason: &str) -> Value {
    json!({ "ok": false, "erreur": reason })
}

pub fn provider_arg(text: &str) -> Result<Option<&'static str>, ()> {
    let t = text.trim().to_lowercase();
    match t.as_str() {
        "" => Ok(None),
        t if t.starts_with("princip") || matches!(t, "primary" | "main") => Ok(Some("primary")),
        t if t.starts_with("secour") || matches!(t, "backup" | "second") => Ok(Some("backup")),
        _ => Err(()),
    }
}

pub fn provider_fr(provider: &str) -> &str {
    match provider {
        "primary" => "principal",
        "backup" => "secours",
        other => other,
    }
}

pub fn channel_ref(env: &ToolEnv, reference: &str) -> Option<String> {
    match reference.trim() {
        "" => None,
        r if r.starts_with('#') => env.deps.scans.resolve_ref(r, env.deps.now()),
        r => Some(r.to_string()),
    }
}

pub fn too_late(env: &ToolEnv, what: &str) -> Option<Value> {
    let left = env.ctx.left_s.filter(|left| *left < LONG_TOOL_MIN_S)?;
    Some(json!({ "status": "refus", "raison": format!("plus assez de temps dans ce message (≈ {} s) pour {what} : réponds avec ce que tu sais déjà", left.max(0.0) as i64) }))
}

pub fn method_missing(env: &ToolEnv, method: &'static str, error: &anyhow::Error) -> bool {
    let missing = crate::peers::is_unsupported(error);
    if missing {
        env.deps.mark_unsupported(method, env.deps.now());
    }
    missing
}

pub fn run_external(env: &mut ToolEnv, module_id: &str, name: &str, args: &Value) -> Value {
    external::call(env, module_id, name, args)
}

#[cfg(test)]
mod tests;
