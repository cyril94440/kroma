//! The `Range` header of a request for a file, read against the file's length.

use axum::http::{header, HeaderMap};

pub(super) enum RangeOutcome {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

pub(super) fn parse_range(headers: &HeaderMap, total_size: u64) -> RangeOutcome {
    let raw = match headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        Some(r) => r,
        None => return RangeOutcome::Full,
    };

    let spec = match raw.strip_prefix("bytes=") {
        Some(s) => s.trim(),
        None => return RangeOutcome::Full,
    };

    // Reject multi-range; serve the whole file instead of erroring.
    if spec.contains(',') {
        return RangeOutcome::Full;
    }

    if total_size == 0 {
        return RangeOutcome::Unsatisfiable;
    }

    let (start_str, end_str) = match spec.split_once('-') {
        Some(parts) => parts,
        None => return RangeOutcome::Full,
    };

    let last = total_size - 1;

    if start_str.is_empty() {
        // Suffix range: bytes=-N → last N bytes.
        let n: u64 = match end_str.parse() {
            Ok(n) if n > 0 => n,
            _ => return RangeOutcome::Unsatisfiable,
        };
        let start = total_size.saturating_sub(n);
        return RangeOutcome::Partial { start, end: last };
    }

    let start: u64 = match start_str.parse() {
        Ok(s) => s,
        Err(_) => return RangeOutcome::Unsatisfiable,
    };

    if start > last {
        return RangeOutcome::Unsatisfiable;
    }

    let end: u64 = if end_str.is_empty() {
        last
    } else {
        match end_str.parse::<u64>() {
            Ok(e) => e.min(last),
            Err(_) => return RangeOutcome::Unsatisfiable,
        }
    };

    if end < start {
        return RangeOutcome::Unsatisfiable;
    }

    RangeOutcome::Partial { start, end }
}
