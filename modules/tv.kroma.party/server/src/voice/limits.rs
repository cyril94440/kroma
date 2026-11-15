use std::collections::BTreeMap;

pub const TTL_MS: i64 = 30_000;
pub const SPEAKING_TTL_MS: i64 = 5_000;
pub const MAX_SDP_BYTES: usize = 16 * 1024;
pub const MAX_CANDIDATE_BYTES: usize = 2 * 1024;
pub const MAX_TOKEN_BYTES: usize = 64;
pub const DEVICE_ID_MIN: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Rate {
    pub count: usize,
    pub window_ms: i64,
}

pub const SIGNALS: Rate = Rate { count: 240, window_ms: 10_000 };
pub const SPEAKING: Rate = Rate { count: 10, window_ms: 1_000 };
pub const JOINS: Rate = Rate { count: 12, window_ms: 60_000 };

#[derive(Debug, Default)]
pub struct Windows {
    hits: BTreeMap<String, Vec<i64>>,
}

impl Windows {
    pub fn allow(&mut self, key: String, rate: Rate, now: i64) -> bool {
        let window = self.hits.entry(key).or_default();
        window.retain(|t| now - t < rate.window_ms);
        if window.len() >= rate.count {
            return false;
        }
        window.push(now);
        true
    }

    pub fn prune(&mut self, now: i64) {
        self.hits.retain(|_, w| w.last().is_some_and(|t| now - t < 60_000));
    }
}

pub fn valid_token(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX_TOKEN_BYTES && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn valid_device_id(s: &str) -> bool {
    s.len() >= DEVICE_ID_MIN && valid_token(s)
}
