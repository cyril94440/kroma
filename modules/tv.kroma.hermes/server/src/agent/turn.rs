use std::sync::Arc;

use serde_json::{json, Value};

use super::deps::Deps;
use crate::blackbox::{clip, mask_json};
use crate::model::{Allowed, Dialogue, Model, ModelError, ToolCall};
use crate::store::Incoming;
use crate::strings;
use crate::tools::{run_external, Backend, Kind, ToolEnv, ToolSet, TurnCtx};

const ANNOUNCED: [&str; 3] = ["switched", "vote_open", "upcoming"];
const FINAL_MARGIN_S: f64 = 25.0;
const HARD_STOP_EXTRA_S: f64 = 30.0;
const RESULT_MAX_CHARS: usize = 20_000;

pub struct Limits {
    pub max_steps: usize,
    pub budget_s: f64,
}

pub struct Turn {
    deps: Arc<Deps>,
    model: Arc<dyn Model>,
    tools: Arc<ToolSet>,
    dialogue: Box<dyn Dialogue>,
    message: Incoming,
    ctx: TurnCtx,
    limits: Limits,
    language: String,
    started: i64,
    steps: usize,
    used: Vec<String>,
    grace: bool,
    last_label: Option<&'static str>,
    failure: Option<ModelError>,
}

fn status_of(out: &Value) -> Option<&str> {
    out.get("status").and_then(Value::as_str)
}

fn refused(out: &Value) -> bool {
    out.get("ok") == Some(&Value::Bool(false)) || status_of(out) == Some("refus") || out.get("error").is_some_and(|e| !e.is_null()) || out.get("erreur").is_some_and(|e| !e.is_null())
}

fn call_key(call: &ToolCall) -> String {
    format!("{}{}", call.name, call.args)
}

fn dedupe(calls: Vec<ToolCall>) -> Vec<ToolCall> {
    let mut seen: Vec<String> = Vec::new();
    calls.into_iter().filter(|c| !seen.contains(&call_key(c)) && { seen.push(call_key(c)); true }).collect()
}

impl Turn {
    #[allow(clippy::too_many_arguments)]
    pub fn new(deps: Arc<Deps>, model: Arc<dyn Model>, tools: Arc<ToolSet>, dialogue: Box<dyn Dialogue>, message: Incoming, limits: Limits, language: String) -> Self {
        let started = deps.now();
        Self { deps, model, tools, dialogue, message, ctx: TurnCtx::default(), limits, language, started, steps: 0, used: Vec::new(), grace: false, last_label: None, failure: None }
    }

    pub fn message(&self) -> &Incoming {
        &self.message
    }

    pub fn answered(&self) -> bool {
        self.ctx.answered
    }

    fn label(&mut self, key: &'static str) {
        if self.last_label == Some(key) {
            return;
        }
        self.last_label = Some(key);
        let text = strings::text(&self.language, &format!("hermes.activity.{key}"), &[]);
        let _ = self.deps.room.activity(Some(&text));
    }

    fn post(&mut self, key: &str) {
        let text = strings::text(&self.language, key, &[]);
        if self.deps.room.say(&text, None, Some(&self.message.id)).is_ok() {
            self.ctx.answered = true;
        }
    }

    fn execute(&mut self, call: &ToolCall) -> Value {
        if let Some(why) = &call.malformed {
            return json!({ "erreur": format!("arguments illisibles pour {} : {why}. Renvoie un objet JSON valide.", call.name) });
        }
        let tools = self.tools.clone();
        let Some(tool) = tools.get(&call.name) else {
            return json!({ "erreur": format!("outil inconnu : {}", call.name) });
        };
        if tool.kind == Kind::Action && self.ctx.diffusion_done {
            return json!({ "status": "refus", "raison": "une seule action de diffusion par message" });
        }
        if tool.kind == Kind::Action && self.ctx.partial_seen {
            return json!({ "status": "refus", "raison": "le guide n'a donné qu'une correspondance PARTIELLE : propose-la avec reply et attends la confirmation (nouveau message) avant de changer l'image de tout le monde" });
        }
        self.label(tool.activity.unwrap_or("thinking"));
        let deps = self.deps.clone();
        let model = self.model.clone();
        let message = self.message.clone();
        let mut env = ToolEnv { deps: &deps, ctx: &mut self.ctx, model: &model, message: &message };
        let out = match &tool.backend {
            Backend::Builtin(handler) => handler(&mut env, &call.args),
            Backend::External(module) => run_external(&mut env, module, &call.name, &call.args),
        };
        if tool.kind == Kind::Action && status_of(&out).is_some_and(|s| ANNOUNCED.contains(&s)) {
            self.ctx.diffusion_done = true;
        }
        if call.name == "find_program" && status_of(&out) == Some("partial_matches") {
            self.ctx.partial_seen = true;
        }
        let out = mask_json(out, &deps.settings().secrets());
        let text = out.to_string();
        if text.chars().count() > RESULT_MAX_CHARS {
            return json!({ "tronque": true, "debut": clip(&text, RESULT_MAX_CHARS) });
        }
        out
    }

    fn kind_of(&self, call: &ToolCall) -> Option<Kind> {
        self.tools.get(&call.name).map(|t| t.kind)
    }

    fn run_batch(&mut self, calls: Vec<ToolCall>) -> (Vec<(ToolCall, Value)>, bool) {
        let unique = dedupe(calls.clone());
        let inv = unique.iter().any(|c| self.kind_of(c) == Some(Kind::Investigation));
        let refusable = unique.iter().any(|c| self.kind_of(c).is_some_and(Kind::is_refusable));
        self.ctx.reply_is_progress = inv;
        self.ctx.progress_posted = false;
        let (finals, rest): (Vec<ToolCall>, Vec<ToolCall>) = unique.into_iter().partition(|c| self.kind_of(c).is_some_and(Kind::is_final));
        let ordered: Vec<ToolCall> = if inv && !refusable { finals.into_iter().chain(rest).collect() } else { rest.into_iter().chain(finals).collect() };
        let (mut done, mut failed, mut ended) = (Vec::<(String, Value)>::new(), false, false);
        for call in &ordered {
            let kind = self.kind_of(call);
            if kind == Some(Kind::Reply) && failed {
                done.push((call_key(call), json!("non posté : l'action du même message a échoué — réponds d'après son résultat")));
                continue;
            }
            let out = self.execute(call);
            self.used.push(call.name.clone());
            if kind == Some(Kind::Reply) && inv {
                self.ctx.progress_any = true;
            }
            match kind {
                Some(Kind::Action) if status_of(&out).is_some_and(|s| ANNOUNCED.contains(&s)) => ended = true,
                Some(Kind::Action) => failed = true,
                Some(k) if k.is_refusable() && refused(&out) => failed = true,
                Some(Kind::Reply) if refused(&out) => failed = true,
                Some(Kind::Ignore) => ended = true,
                Some(Kind::Reply) if !inv => ended = true,
                _ => {}
            }
            done.push((call_key(call), out));
        }
        let results = calls
            .into_iter()
            .map(|c| {
                let out = done.iter().find(|(k, _)| *k == call_key(&c)).map_or_else(|| json!("non exécuté"), |(_, v)| v.clone());
                (c, out)
            })
            .collect();
        (results, ended && !failed && !inv)
    }

    /// Runs one step: one model call and the tools it asked for. `Some` is the
    /// reason the turn ended, `None` means another step is due.
    pub fn advance(&mut self) -> Option<&'static str> {
        let step = self.steps + 1;
        if step > self.limits.max_steps {
            return Some(self.finish("etapes"));
        }
        let elapsed = (self.deps.now() - self.started) as f64 / 1000.0;
        if elapsed > self.limits.budget_s + HARD_STOP_EXTRA_S {
            if step == 1 || self.grace {
                return Some(self.finish("budget"));
            }
            self.grace = true;
        }
        let final_only = self.grace || step == self.limits.max_steps || elapsed >= self.limits.budget_s - FINAL_MARGIN_S;
        if self.steps == 0 {
            self.label("thinking");
        }
        let allowed = if final_only { Allowed::Only(self.tools.final_names()) } else { Allowed::Any };
        let started = self.deps.now();
        let stepped = self.dialogue.step(&allowed);
        let took = self.deps.now() - started;
        self.deps.audit("llm", "call", json!({ "step": step, "ms": took, "ok": stepped.is_ok(), "final_only": final_only, "error": stepped.as_ref().err().map(ToString::to_string) }));
        self.steps = step;
        let mut calls = match stepped {
            Ok(calls) => calls,
            Err(error) => {
                self.deps.audit("audit", "agent_error", json!({ "message": self.message.id, "error": error.to_string() }));
                self.failure = Some(error);
                return Some(self.finish("erreur"));
            }
        };
        if final_only {
            let dropped: Vec<String> = calls.iter().filter(|c| !self.kind_of(c).is_some_and(Kind::is_final)).map(|c| c.name.clone()).collect();
            if !dropped.is_empty() {
                self.deps.audit("audit", "agent_final_only", json!({ "dropped": dropped }));
            }
            calls.retain(|c| self.kind_of(c).is_some_and(Kind::is_final));
        }
        if calls.is_empty() {
            return Some(self.finish("sans_outil"));
        }
        let (results, terminal) = self.run_batch(calls);
        if terminal {
            return Some(self.finish("terminal"));
        }
        self.dialogue.answer(&results);
        None
    }

    pub fn finish(&mut self, fin: &'static str) -> &'static str {
        if !self.ctx.answered {
            if fin == "erreur" && self.used.is_empty() && matches!(self.failure, Some(ModelError::RateLimited | ModelError::Transient(_))) {
                if self.deps.take_notice_slot(self.deps.now()) {
                    self.post("hermes.fallback.unavailable");
                }
            } else if fin != "terminal" && self.used.iter().any(|n| n != "ignore") {
                self.post("hermes.fallback.timeout");
            } else if self.ctx.progress_any {
                self.post("hermes.fallback.nothing");
            }
        }
        let seconds = (self.deps.now() - self.started) as f64 / 1000.0;
        self.deps.audit("audit", "agent_turn", json!({ "message": self.message.id, "steps": self.steps, "s": (seconds * 10.0).round() / 10.0, "tools": self.used, "fin": fin, "answered": self.ctx.answered }));
        let _ = self.deps.room.activity(None);
        fin
    }

    pub fn abort(&mut self, fin: &'static str) -> &'static str {
        self.finish(fin)
    }
}
