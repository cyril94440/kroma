//! `POST /api/discover/{kind}/{tmdb_id}/watch-now`: hand the viewer a title the
//! library does not hold, as a library item, within seconds.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::error::lerr;
use crate::api::extract::AuthUser;
use crate::api::point;
use crate::i18n;
use crate::model::RequestKind;
use crate::services::watch_now::{self, Outcome, Reason, Scope};
use crate::state::SharedState;

#[derive(Debug, Default, Deserialize)]
pub struct Body {
    #[serde(default)]
    season: Option<u32>,
    #[serde(default)]
    episode: Option<u32>,
}

pub async fn watch_now(
    State(state): State<SharedState>,
    AuthUser(user): AuthUser,
    Path((kind, tmdb_id)): Path<(String, u64)>,
    body: Option<Json<Body>>,
) -> Result<Response, Response> {
    let loc = i18n::user_locale(&user);
    if !watch_now::may_watch_now(&user) {
        return Err(lerr(loc, StatusCode::NOT_FOUND, "error.itemNotFound"));
    }
    point::require(&state, &user, watch_now::ACQUISITION)?;
    let scope = scope_of(&kind, tmdb_id, body.map(|Json(b)| b).unwrap_or_default())
        .map_err(|key| lerr(loc, StatusCode::BAD_REQUEST, key))?;
    match watch_now::watch_now(&state, &user, &scope).await {
        Ok(outcome) => Ok(Json(answer(outcome, loc)).into_response()),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "watch now failed");
            Err(lerr(loc, StatusCode::BAD_GATEWAY, "error.watchNow.failed"))
        }
    }
}

fn scope_of(kind: &str, tmdb_id: u64, body: Body) -> Result<Scope, &'static str> {
    if tmdb_id == 0 {
        return Err("error.watchNow.tmdbRequired");
    }
    match kind {
        "movie" => Ok(Scope {
            kind: RequestKind::Movie,
            tmdb_id,
            season: None,
            episode: None,
        }),
        "tv" | "show" => match (body.season, body.episode) {
            (None, None) => Ok(Scope {
                kind: RequestKind::Show,
                tmdb_id,
                season: Some(1),
                episode: Some(1),
            }),
            (Some(season), Some(episode)) if season > 0 && episode > 0 => Ok(Scope {
                kind: RequestKind::Show,
                tmdb_id,
                season: Some(season),
                episode: Some(episode),
            }),
            _ => Err("error.watchNow.episodeRequired"),
        },
        _ => Err("error.watchNow.unknownMedia"),
    }
}

fn answer(outcome: Outcome, locale: &'static str) -> serde_json::Value {
    match outcome {
        Outcome::Ready { item_id, file_id } => {
            json!({ "status": "ready", "itemId": item_id, "fileId": file_id })
        }
        Outcome::Requested { request_id } => {
            json!({ "status": "requested", "requestId": request_id })
        }
        Outcome::Unavailable { reason, request_id } => {
            let (code, key) = match reason {
                Reason::NoEngine => ("noEngine", "error.watchNow.noEngine"),
                Reason::NoIndexer => ("noIndexer", "error.watchNow.noIndexer"),
                Reason::NoRelease => ("noRelease", "error.watchNow.noRelease"),
            };
            json!({
                "status": "unavailable",
                "reason": code,
                "message": i18n::t(locale, key, &[]),
                "requestId": request_id,
            })
        }
    }
}

#[cfg(test)]
mod tests;
