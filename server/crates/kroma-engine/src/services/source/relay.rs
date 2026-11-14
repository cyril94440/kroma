use std::fmt;
use std::io::{Error, Result};

use tokio::io::AsyncReadExt;

use kroma_http::Fetch;
use kroma_primitives::input::RemoteUrl;

use super::{Sources, Upstream};

/// What an upstream host answered to a range request it did not honour. It
/// names a status and never the link.
#[derive(Debug)]
pub struct UpstreamStatus(pub u16);

impl fmt::Display for UpstreamStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the source's host answered {} to a range request",
            self.0
        )
    }
}

impl std::error::Error for UpstreamStatus {}

/// Whether `error` says the link itself is no good any more, so a fresh one
/// should be asked for.
pub fn link_is_stale(error: &Error) -> bool {
    error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<UpstreamStatus>())
        .is_some_and(|status| matches!(status.0, 401 | 403 | 404 | 410))
}

struct Granted {
    start: u64,
    end: u64,
    total: Option<u64>,
}

fn content_range(value: &str) -> Option<Granted> {
    let (span, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (start, end) = span.split_once('-')?;
    Some(Granted {
        start: start.trim().parse().ok()?,
        end: end.trim().parse().ok()?,
        total: total.trim().parse().ok(),
    })
}

fn mismatch(what: &str) -> Error {
    Error::other(format!(
        "the source's host answered a range request with {what}"
    ))
}

impl Sources {
    /// The bytes `[start, end]` of the file of `size` bytes behind `url`, read as
    /// they arrive. The request leaves the way every other outbound call of the
    /// core does, through the network policy, and only to a public address,
    /// which curl is pinned to once it is checked; it follows no redirect.
    /// Dropping the reader ends the transfer. An upstream that answers anything
    /// but exactly the range asked for, of a file of the size the stub
    /// declares, is an error that names a status and never the link.
    pub async fn relay(
        &self,
        url: &RemoteUrl,
        start: u64,
        end: u64,
        size: u64,
    ) -> Result<Upstream> {
        let streamed = Fetch::new()
            .purpose(super::vet::PURPOSE)
            .header("range", format!("bytes={start}-{end}"))
            .no_redirects()
            .public_only()
            .https_only()
            .stream(url.expose())
            .await
            .map_err(|error| Error::other(url.redact(&format!("{error:#}"))))?;
        let length = end - start + 1;
        let declared = streamed
            .header("content-length")
            .and_then(|text| text.trim().parse::<u64>().ok());
        match streamed.status {
            206 => {
                let granted = streamed
                    .header("content-range")
                    .and_then(content_range)
                    .ok_or_else(|| mismatch("no usable Content-Range"))?;
                if granted.start != start || granted.end != end {
                    return Err(mismatch("another range"));
                }
                if granted.total.is_some_and(|total| total != size) {
                    return Err(mismatch("a file of another size"));
                }
                if declared.is_some_and(|declared| declared != length) {
                    return Err(mismatch("another length"));
                }
            }
            200 if start == 0 => {
                if declared.is_some_and(|declared| declared != size) {
                    return Err(mismatch("a file of another size"));
                }
            }
            status => return Err(Error::other(UpstreamStatus(status))),
        }
        Ok(Box::pin(streamed.take(length)))
    }
}
