use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use kroma_module_sdk::host::{port_reply, HostCtx};

use crate::runtime::Sport;
use crate::standings::{LiveGame, StandingRow};

mod declarations;

pub use declarations::declared;

pub const TOOLS: &str = "tv.kroma.hermes/tools";

#[derive(Serialize)]
pub struct Listed {
    pub tools: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Call {
    pub name: String,
    pub arguments: Value,
    pub context: Value,
}

impl Call {
    fn requested_by(&self) -> String {
        text(&self.context, "userId")
    }
}

#[derive(Serialize)]
pub struct Answered {
    pub result: Value,
}

pub fn routes<S: HostCtx + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route(&format!("/_port/{TOOLS}/list"), post(list::<S>))
        .route(&format!("/_port/{TOOLS}/call"), post(call::<S>))
}

async fn list<S: HostCtx + Clone>(
    State(_): State<S>,
    Json(_): Json<Value>,
) -> Json<Result<Listed, String>> {
    port_reply(|| Ok(Listed { tools: declared() })).await
}

async fn call<S: HostCtx + Clone>(
    State(_): State<S>,
    Extension(sport): Extension<Arc<Sport>>,
    Json(call): Json<Call>,
) -> Json<Result<Answered, String>> {
    port_reply(move || {
        Ok(Answered {
            result: dispatch(&sport, &call.name, &call.arguments, &call.requested_by()),
        })
    })
    .await
}

fn text(arguments: &Value, key: &str) -> String {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn list_of<T: serde::de::DeserializeOwned>(arguments: &Value, key: &str) -> Option<Vec<T>> {
    serde_json::from_value(arguments.get(key)?.clone()).ok()
}

pub fn dispatch(sport: &Sport, name: &str, arguments: &Value, requested_by: &str) -> Value {
    match name {
        "match_facts" => {
            sport.tool_match_facts(&text(arguments, "equipes"), &text(arguments, "competition"))
        }
        "compos_match" => sport.tool_compos_match(&text(arguments, "equipes")),
        "live_standings" => sport.live_standings(
            &text(arguments, "competition"),
            list_of::<StandingRow>(arguments, "standings"),
            list_of::<LiveGame>(arguments, "matches"),
            arguments.get("top").and_then(Value::as_i64).unwrap_or(10),
        ),
        "start_prono" => {
            if sport.require_admin_id(requested_by).is_err() {
                return json!({
                    "ok": false,
                    "status": "refus",
                    "error": "adminOnly",
                    "raison": "reserve_aux_admins",
                    "message": "le prono à la demande est réservé aux administrateurs de la maison, vérifié côté serveur : dis-le simplement, sans insister ni contourner"
                });
            }
            match sport.start_prono(&text(arguments, "home"), &text(arguments, "away")) {
                Ok(done) => done,
                Err(reason) => json!({"ok": false, "error": reason}),
            }
        }
        "sport_channel_hint" => match sport.note_channel_hint(
            &text(arguments, "home"),
            &text(arguments, "away"),
            &text(arguments, "answer"),
        ) {
            Ok(event) => json!({"ok": true, "event": event}),
            Err(reason) => json!({"ok": false, "error": reason}),
        },
        "calibrate_clock" => match sport.calibrate(&text(arguments, "clock"), None) {
            Ok(done) => done,
            Err(e) => json!({"ok": false, "error": e.code()}),
        },
        other => json!({"error": format!("outil inconnu : {other}")}),
    }
}

#[cfg(test)]
mod tests;
