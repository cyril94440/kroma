use std::io::{Error, Result};

use kroma_http::Fetch;
use kroma_primitives::input::RemoteUrl;

use super::{Sources, Upstream};

impl Sources {
    /// The bytes `[start, end]` of the file behind `url`, read as they arrive.
    /// The request leaves the way every other outbound call of the core does,
    /// through the network policy, and only to a public address, which curl is
    /// pinned to once it is checked; it follows no redirect. Dropping the
    /// reader ends the transfer. An upstream that answers anything but the range
    /// asked for is an error that names a status and never the link.
    pub async fn relay(&self, url: &RemoteUrl, start: u64, end: u64) -> Result<Upstream> {
        let mut fetch = Fetch::new()
            .purpose(super::vet::PURPOSE)
            .header("range", format!("bytes={start}-{end}"))
            .no_redirects();
        if !self.vetting.is_insecure() {
            fetch = fetch.public_only().https_only();
        }
        let streamed = fetch
            .stream(url.expose())
            .await
            .map_err(|error| Error::other(url.redact(&format!("{error:#}"))))?;
        let honoured = match streamed.status {
            206 => streamed
                .header("content-range")
                .and_then(|range| range.strip_prefix("bytes "))
                .and_then(|range| range.split(['-', '/']).next())
                .and_then(|first| first.parse::<u64>().ok())
                == Some(start),
            200 => start == 0,
            _ => false,
        };
        if !honoured {
            return Err(Error::other(format!(
                "the source's host answered {} to a range request",
                streamed.status
            )));
        }
        Ok(Box::pin(streamed))
    }
}
