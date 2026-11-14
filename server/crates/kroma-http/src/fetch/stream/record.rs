//! The journal row a [`super::Streamed`] files: when it is blocked, when it
//! fails before its headers, and when the body is dropped.

use std::time::Instant;

use crate::egress::{self, Exchange, Route, BLOCKED};

use super::Streamed;

pub(super) struct Record {
    pub(super) purpose: &'static str,
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) route: Route,
    pub(super) rule: Option<String>,
    pub(super) started: Instant,
    pub(super) status: u16,
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
                duration_ms: u64::try_from(record.started.elapsed().as_millis())
                    .unwrap_or(u64::MAX),
                error: None,
                rule: record.rule,
                carried: egress::gateway().is_some(),
            });
        }
    }
}

pub(super) fn blocked(
    purpose: &'static str,
    host: String,
    port: u16,
    rule: Option<String>,
    reason: &str,
) -> Exchange {
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

pub(super) fn failed(
    purpose: &'static str,
    host: String,
    port: u16,
    route: Route,
    rule: Option<String>,
    started: Instant,
    reason: &str,
) -> Exchange {
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

