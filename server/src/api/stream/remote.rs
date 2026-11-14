//! A title held by a module: turning its library path into the input a route
//! reads, and the errors a viewer sees when its source cannot supply one.

use axum::http::StatusCode;
use axum::response::Response;

use crate::services::source::is_stub;

use crate::api::error::{json_error, lerr};
use crate::model::User;
use crate::services::source::{Resolved, SourceError};
use crate::state::SharedState;

pub(crate) fn locale_of(user: &User) -> &'static str {
    user.language
        .as_deref()
        .and_then(crate::i18n::normalize)
        .unwrap_or(crate::i18n::DEFAULT_LOCALE)
}

fn failure(user: &User, error: &SourceError) -> Response {
    crate::services::source::log_failure("playback", error);
    let (status, key) = match error {
        SourceError::NoModule => (StatusCode::SERVICE_UNAVAILABLE, "error.sourceDisabled"),
        SourceError::Unreachable => (StatusCode::BAD_GATEWAY, "error.sourceUnreachable"),
        SourceError::Gone(_) => (StatusCode::BAD_GATEWAY, "error.sourceGone"),
        SourceError::BadStub(_) | SourceError::Refused(_) => {
            (StatusCode::BAD_GATEWAY, "error.sourceRefused")
        }
    };
    lerr(locale_of(user), status, key)
}

pub(crate) fn unreachable(user: &User) -> Response {
    lerr(locale_of(user), StatusCode::BAD_GATEWAY, "error.sourceUnreachable")
}

enum Looked {
    Missing,
    Answered(Result<Resolved, SourceError>),
}

/// What a route reads for the library file at `abs`: the file itself, once a stat
/// says it is there, or what a stub's source answers, which `warm` asks to start
/// caching. A mount that is offline and a source that is down are each answered
/// rather than left to a spawn that fails: the first as a plain 404, the second
/// in the viewer's language.
pub(crate) async fn playable(
    state: &SharedState,
    user: &User,
    abs: &str,
    warm: bool,
) -> Result<Resolved, Response> {
    let held = state.clone();
    let path = abs.to_string();
    let looked = tokio::task::spawn_blocking(move || {
        if !is_stub(std::path::Path::new(&path)) && !std::path::Path::new(&path).exists() {
            return Looked::Missing;
        }
        Looked::Answered(held.sources.resolve(&path, warm))
    })
    .await
    .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "source lookup failed"))?;
    match looked {
        Looked::Missing => Err(json_error(
            StatusCode::NOT_FOUND,
            "media file unavailable (mount offline?)",
        )),
        Looked::Answered(Ok(resolved)) => Ok(resolved),
        Looked::Answered(Err(error)) => Err(failure(user, &error)),
    }
}
