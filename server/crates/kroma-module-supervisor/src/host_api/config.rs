//! The config half of the callback API: a module reads the values of the
//! `config` fields its own manifest declares, as the operator saved them in
//! Admin, and nothing else.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use kroma_module_host::{json_error, HostCtx};
use serde_json::{json, Value};

use super::Caller;

#[derive(serde::Deserialize)]
pub(super) struct ConfigQuery {
    key: String,
}

fn refused(caller: &Caller, key: &str, why: &str) -> Response {
    let refusal = format!("config: \"{key}\" {why}");
    tracing::warn!(module = %caller.name(), key = %key, "a module reached for config it does not declare");
    caller.say(&format!("WARN {refusal}"));
    json_error(StatusCode::FORBIDDEN, &refusal)
}

pub(super) async fn get_config<S: HostCtx>(
    State(host): State<S>,
    Extension(caller): Extension<Caller>,
    Query(q): Query<ConfigQuery>,
) -> Response {
    let Some(module) = caller.module() else {
        return refused(
            &caller,
            &q.key,
            "is not reachable by a caller the token does not name",
        );
    };
    let Some(field) = caller.config_field(&q.key) else {
        return refused(
            &caller,
            &q.key,
            "is not declared; add it to config in module.json",
        );
    };
    let value = host
        .module_config(module)
        .get(&q.key)
        .cloned()
        .or_else(|| field.default.map(Value::String))
        .unwrap_or(Value::Null);
    Json(json!({ "value": value })).into_response()
}
