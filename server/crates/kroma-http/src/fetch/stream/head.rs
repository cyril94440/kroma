//! Reading the status line and headers curl dumps ahead of a streamed body.

use anyhow::{bail, Context as _, Result};
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const MAX_HEADER_BYTES: usize = 64 * 1024;

pub(super) async fn read_head<R: AsyncBufRead + Unpin>(
    body: &mut R,
) -> Result<(u16, Vec<(String, String)>)> {
    let mut seen = 0usize;
    loop {
        let mut status = None;
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            let read = body
                .read_line(&mut line)
                .await
                .context("read the response headers")?;
            if read == 0 {
                bail!("the connection closed before the response headers");
            }
            seen += read;
            if seen > MAX_HEADER_BYTES {
                bail!("the response headers are too large");
            }
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                break;
            }
            if let Some(rest) = line.strip_prefix("HTTP/") {
                status = rest
                    .split_whitespace()
                    .nth(1)
                    .and_then(|code| code.parse::<u16>().ok());
            } else if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_string(), value.trim().to_string()));
            }
        }
        match status {
            Some(code) if (100..200).contains(&code) => continue,
            Some(code) => return Ok((code, headers)),
            None => bail!("no HTTP status line in the response"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn head(raw: &str) -> Result<(u16, Vec<(String, String)>)> {
        read_head(&mut tokio::io::BufReader::new(raw.as_bytes())).await
    }

    #[tokio::test]
    async fn the_origin_s_answer_is_the_status_when_nothing_precedes_it() {
        let (status, headers) =
            head("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 0-9/10\r\n\r\n")
                .await
                .unwrap();

        assert_eq!(status, 206);
        assert_eq!(
            headers,
            [("Content-Range".to_string(), "bytes 0-9/10".to_string())]
        );
    }

    #[tokio::test]
    async fn an_interim_answer_is_skipped_for_the_one_after_it() {
        let (status, _) = head("HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\n\r\n")
            .await
            .unwrap();

        assert_eq!(status, 200);
    }

    #[tokio::test]
    async fn a_proxy_s_connection_established_would_be_read_as_the_answer() {
        let (status, _) =
            head("HTTP/1.1 200 Connection Established\r\n\r\nHTTP/1.1 403 Forbidden\r\n\r\n")
                .await
                .unwrap();

        assert_eq!(status, 200, "which is why the stream asks curl to hide it");
    }

    #[tokio::test]
    async fn a_head_that_ends_early_or_runs_on_is_an_error() {
        assert!(head("HTTP/1.1 200 OK\r\nA: b\r\n").await.is_err());
        assert!(head("").await.is_err());
        let huge = format!(
            "HTTP/1.1 200 OK\r\n{}\r\n\r\n",
            format!("X: {}\r\n", "a".repeat(1000)).repeat(100)
        );
        assert!(head(&huge).await.is_err());
    }
}
