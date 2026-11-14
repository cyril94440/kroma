//! A title held by a module: turning its library path into the input a route
//! reads, and the errors a viewer sees when its source cannot supply one.

use axum::http::StatusCode;
use axum::response::Response;

use crate::api::error::{json_error, lerr};
use crate::model::User;
use crate::services::source::{Resolved, SourceError, Unplayable};
use crate::state::SharedState;

pub(crate) fn locale_of(user: &User) -> &'static str {
    user.language
        .as_deref()
        .and_then(crate::i18n::normalize)
        .unwrap_or(crate::i18n::DEFAULT_LOCALE)
}

pub(crate) fn failure(user: &User, error: &SourceError) -> Response {
    crate::services::source::log_failure("playback", error);
    let (status, key) = failure_key(error);
    lerr(locale_of(user), status, key)
}

pub(crate) fn failure_key(error: &SourceError) -> (StatusCode, &'static str) {
    match error {
        SourceError::NoModule => (StatusCode::SERVICE_UNAVAILABLE, "error.sourceDisabled"),
        SourceError::Unreachable => (StatusCode::BAD_GATEWAY, "error.sourceUnreachable"),
        SourceError::Gone(_) => (StatusCode::BAD_GATEWAY, "error.sourceGone"),
        SourceError::Preparing => (StatusCode::SERVICE_UNAVAILABLE, "error.sourcePreparing"),
        SourceError::BadStub(_) | SourceError::Refused(_) => {
            (StatusCode::BAD_GATEWAY, "error.sourceRefused")
        }
    }
}

pub(crate) fn unreachable(user: &User) -> Response {
    lerr(locale_of(user), StatusCode::BAD_GATEWAY, "error.sourceUnreachable")
}

/// What a route reads for the library file at `abs`, answered in the viewer's
/// language when it cannot be had. `warm` asks a stub's source to start caching.
pub(crate) async fn playable(
    state: &SharedState,
    user: &User,
    abs: &str,
    warm: bool,
) -> Result<Resolved, Response> {
    let held = state.clone();
    let path = abs.to_string();
    let looked = tokio::task::spawn_blocking(move || held.sources.playable(&path, warm))
        .await
        .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "source lookup failed"))?;
    looked.map_err(|why| match why {
        Unplayable::Missing => json_error(StatusCode::NOT_FOUND, "media file unavailable (mount offline?)"),
        Unplayable::Source(error) => failure(user, &error),
    })
}

/// What an embedded-subtitle extraction reads: a file on this machine, which for
/// a stub is its complete local copy. A cue is interleaved with the whole title,
/// so a title not cached yet is never demuxed over the network: its source is
/// asked to cache it and the viewer is told to come back.
pub(crate) async fn embedded(
    state: &SharedState,
    user: &User,
    abs: &str,
) -> Result<Resolved, Response> {
    if !crate::services::source::is_stub(std::path::Path::new(abs)) {
        return playable(state, user, abs, false).await;
    }
    let held = state.clone();
    let path = abs.to_string();
    let copy = tokio::task::spawn_blocking(move || held.sources.local_warming(&path))
        .await
        .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "source lookup failed"))?;
    match copy {
        Ok(Some(copy)) => Ok(Resolved { input: kroma_media::input::MediaInput::Local(copy), stub: None }),
        Ok(None) => Err(lerr(locale_of(user), StatusCode::SERVICE_UNAVAILABLE, "error.subtitleNotCached")),
        Err(error) => Err(failure(user, &error)),
    }
}
