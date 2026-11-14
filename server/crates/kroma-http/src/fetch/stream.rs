//! [`Fetch::stream`]: one GET whose body is read as it arrives rather than held.

use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};
use std::time::Instant;

use anyhow::{anyhow, bail, Context as _, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, ReadBuf};
use tokio::process::{Child, ChildStdout, Command};

use crate::address::WebTarget;
use crate::config::option_line;
use crate::egress::{self, scrubbed, Exchange, Route, Ruling, BLOCKED};

use super::render::{gateway_lines, proxy_lines};
use super::send::chosen_proxy;
use super::Fetch;

const CONNECT_SECONDS: u32 = 15;
const STALLED_SECONDS: u32 = 30;
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// A response whose body is still on its way: the status and headers are read,
/// the bytes come out of [`AsyncRead`]. Dropping it kills the curl process
/// behind it, so a reader that goes away leaves nothing running, and files the
/// exchange with what was read.
pub struct Streamed {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    body: BufReader<ChildStdout>,
    _child: Child,
    record: Option<Record>,
    bytes_in: u64,
}

struct Record {
    purpose: &'static str,
    host: String,
    port: u16,
    route: Route,
    rule: Option<String>,
    started: Instant,
    status: u16,
}

impl Streamed {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

impl AsyncRead for Streamed {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let poll = Pin::new(&mut self.body).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            self.bytes_in += (buf.filled().len() - before) as u64;
        }
        poll
    }
}

impl Drop for Streamed {
    fn drop(&mut self) {
        if let Some(record) = self.record.take() {
            egress::record(Exchange {
                purpose: record.purpose,
                method: "GET",
                host: record.host,
                port: record.port,
                route: record.route,
                status: Some(record.status),
                bytes_in: self.bytes_in,
                bytes_out: 0,
                duration_ms: u64::try_from(record.started.elapsed().as_millis()).unwrap_or(u64::MAX),
                error: None,
                rule: record.rule,
                carried: egress::gateway().is_some(),
            });
        }
    }
}

impl Fetch {
    /// GET `url` and return as soon as the response headers are in, with the
    /// body left to read. For a body too large to hold that a caller relays: it
    /// has no overall time limit, only a connect limit and a stall limit, so the
    /// request's own headers (`Range`) decide how much arrives. The call passes
    /// through the same gate, proxy and address checks as any other.
    pub async fn stream(&self, url: &str) -> Result<Streamed> {
        let started = Instant::now();
        let (target, Ruling { decision, rule }) = egress::rule_url(self.purpose, "GET", url);
        let host_port = |target: &Option<WebTarget>| {
            target.as_ref().map_or((String::new(), 0), |t| (t.host.clone(), t.port))
        };
        let proxy = match chosen_proxy(decision, self.socks5.as_deref()) {
            Ok(proxy) => proxy,
            Err(reason) => {
                let (host, port) = host_port(&target);
                egress::record(blocked(self.purpose, host, port, rule, &reason));
                bail!("{BLOCKED}: {reason}");
            }
        };
        let mut config = self.base_config();
        config.push_str(&option_line("max-time", "0"));
        config.push_str(&option_line("connect-timeout", &CONNECT_SECONDS.to_string()));
        config.push_str(&option_line("speed-limit", "1"));
        config.push_str(&option_line("speed-time", &STALLED_SECONDS.to_string()));
        config.push_str(&option_line("dump-header", "-"));
        config.push_str(&option_line("url", url));
        let route = if let Some(gateway) = egress::gateway() {
            let username = egress::Gateway::username(self.purpose, self.public_only, self.socks5.is_some());
            config.push_str(&gateway_lines(gateway, &username));
            Route::Proxy
        } else {
            if self.public_only {
                config.push_str(&crate::pin::public_resolve_line(url)?);
            }
            if let Some(proxy) = &proxy {
                config.push_str(&proxy_lines(proxy));
            }
            if proxy.is_some() { Route::Proxy } else { Route::Direct }
        };
        let (host, port) = host_port(&target);
        let mut child = Command::new("curl")
            .arg("-q")
            .arg("--config")
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("spawn curl")?;
        let mut stdin = child.stdin.take().context("curl stdin was not piped")?;
        stdin.write_all(config.as_bytes()).await.context("write the curl config")?;
        drop(stdin);
        let stdout = child.stdout.take().context("curl stdout was not piped")?;
        let mut stderr = child.stderr.take();
        let mut body = BufReader::new(stdout);
        let (status, headers) = match read_head(&mut body).await {
            Ok(head) => head,
            Err(read) => {
                let mut said = String::new();
                if let Some(stderr) = stderr.as_mut() {
                    let _ = stderr.take(4096).read_to_string(&mut said).await;
                }
                let reason = scrubbed(said.trim(), url, &host);
                egress::record(failed(self.purpose, host, port, route, rule, started, &reason));
                return Err(anyhow!("{}", if reason.is_empty() { read.to_string() } else { reason }));
            }
        };
        Ok(Streamed {
            status,
            headers,
            body,
            _child: child,
            record: Some(Record {
                purpose: self.purpose,
                host,
                port,
                route,
                rule,
                started,
                status,
            }),
            bytes_in: 0,
        })
    }
}

async fn read_head(body: &mut BufReader<ChildStdout>) -> Result<(u16, Vec<(String, String)>)> {
    let mut seen = 0usize;
    loop {
        let mut status = None;
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            let read = body.read_line(&mut line).await.context("read the response headers")?;
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
                status = rest.split_whitespace().nth(1).and_then(|code| code.parse::<u16>().ok());
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

fn blocked(purpose: &'static str, host: String, port: u16, rule: Option<String>, reason: &str) -> Exchange {
    Exchange {
        purpose,
        method: "GET",
        host,
        port,
        route: Route::Blocked,
        status: None,
        bytes_in: 0,
        bytes_out: 0,
        duration_ms: 0,
        error: Some(format!("{BLOCKED}: {reason}")),
        rule,
        carried: false,
    }
}

fn failed(purpose: &'static str, host: String, port: u16, route: Route, rule: Option<String>, started: Instant, reason: &str) -> Exchange {
    Exchange {
        purpose,
        method: "GET",
        host,
        port,
        route,
        status: None,
        bytes_in: 0,
        bytes_out: 0,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        error: Some(reason.to_string()),
        rule,
        carried: egress::gateway().is_some(),
    }
}

#[cfg(test)]
mod tests;
