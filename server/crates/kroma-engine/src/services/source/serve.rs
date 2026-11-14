use std::fmt;

use axum::http::HeaderMap;
use axum::response::Response;

use kroma_media::stream::{content_type_of, stream_open, stream_ranged};
use kroma_media::ByteMeter;

use super::relay::link_is_stale;
use super::vet::Place;
use super::{SourceError, Sources};

/// Why a title could not be served.
#[derive(Debug)]
pub enum ServeError {
    Source(SourceError),
    Upstream(std::io::Error),
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(f, "{error}"),
            Self::Upstream(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ServeError {}

impl Sources {
    /// The bytes of the library file at `abs`, honouring `Range` and `HEAD`: from
    /// the complete local copy when the source has one, else relayed from its
    /// host. A link the host calls gone is asked for again once.
    pub async fn serve<M: ByteMeter>(
        &self,
        abs: &str,
        warm: bool,
        head: bool,
        headers: &HeaderMap,
        meter: M,
    ) -> Result<Response, ServeError> {
        let me = self
            .me
            .upgrade()
            .ok_or(ServeError::Source(SourceError::Unreachable))?;
        let mut again = true;
        loop {
            let (held, path) = (me.clone(), abs.to_string());
            let found = tokio::task::spawn_blocking(move || held.find(&path, warm))
                .await
                .map_err(|_| ServeError::Source(SourceError::Unreachable))?
                .map_err(ServeError::Source)?;
            let extension = found.stub.container();
            match found.place {
                Place::Copy(copy) => {
                    let kind = extension
                        .clone()
                        .or_else(|| {
                            copy.path
                                .extension()
                                .and_then(|e| e.to_str())
                                .map(str::to_ascii_lowercase)
                        })
                        .unwrap_or_default();
                    match tokio::task::spawn_blocking(move || copy.open()).await {
                        Ok(Ok(file)) => {
                            let file = tokio::fs::File::from_std(file);
                            return Ok(
                                stream_open(file, content_type_of(&kind), headers, meter).await
                            );
                        }
                        _ if again => {
                            self.memo.forget(abs);
                            again = false;
                        }
                        _ => {
                            return Err(ServeError::Source(SourceError::Gone(
                                "the copy is gone".into(),
                            )))
                        }
                    }
                }
                Place::Upstream(url) => {
                    let size = found.stub.size;
                    let kind = extension.unwrap_or_default();
                    let reader = me.clone();
                    let opened = stream_ranged(
                        size,
                        content_type_of(&kind),
                        headers,
                        head,
                        meter.clone(),
                        |start, end| async move { reader.relay(&url, start, end, size).await },
                    )
                    .await;
                    match opened {
                        Ok(response) => return Ok(response),
                        Err(error) if link_is_stale(&error) => {
                            self.memo.forget(abs);
                            if !again {
                                return Err(ServeError::Upstream(error));
                            }
                            again = false;
                        }
                        Err(error) => return Err(ServeError::Upstream(error)),
                    }
                }
            }
        }
    }
}
