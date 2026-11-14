//! Direct play of a title a module holds: the stub's bytes, from the local copy
//! when the source has one and otherwise relayed from the host it points at.

use axum::http::HeaderMap;
use axum::response::Response;

use kroma_media::ByteMeter;

use crate::model::User;
use crate::services::source::ServeError;
use crate::state::SharedState;

use super::remote::{failure, unreachable};

pub(super) async fn stream_stub<M: ByteMeter>(
    state: &SharedState,
    viewer: &User,
    path: &str,
    head: bool,
    headers: &HeaderMap,
    meter: M,
) -> Result<Response, Response> {
    state
        .sources
        .serve(path, true, head, headers, meter)
        .await
        .map_err(|error| match error {
            ServeError::Source(error) => failure(viewer, &error),
            ServeError::Upstream(error) => {
                tracing::warn!(%error, "a streamed title could not be opened at its host");
                unreachable(viewer)
            }
        })
}
