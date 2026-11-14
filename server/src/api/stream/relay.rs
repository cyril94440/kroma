//! Direct play of a title a module holds: the stub's bytes, from the local copy
//! when the source has one and otherwise relayed from the host it points at.

use axum::http::HeaderMap;
use axum::response::Response;

use kroma_media::input::MediaInput;
use kroma_media::stream::{content_type_of, stream_file, stream_ranged};
use kroma_media::ByteMeter;

use crate::model::User;
use crate::state::SharedState;

use super::remote::{playable, unreachable};

pub(super) async fn stream_stub<M: ByteMeter>(
    state: &SharedState,
    viewer: &User,
    path: &str,
    head: bool,
    headers: &HeaderMap,
    meter: M,
) -> Result<Response, Response> {
    let resolved = playable(state, viewer, path, true).await?;
    match resolved.input {
        MediaInput::Local(copy) => Ok(stream_file(&copy, headers, meter).await),
        MediaInput::Remote(url) => {
            let stub = resolved.stub.ok_or_else(|| unreachable(viewer))?;
            let container = stub.container().unwrap_or_default();
            let sources = state.sources.clone();
            stream_ranged(
                stub.size,
                content_type_of(&container),
                headers,
                head,
                meter,
                |start, end| async move { sources.relay(&url, start, end).await },
            )
            .await
            .map_err(|error| {
                tracing::warn!(%error, "a streamed title could not be opened at its host");
                unreachable(viewer)
            })
        }
    }
}
