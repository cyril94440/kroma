use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

const UNKNOWN_MAGNET: &str = "MAGNET_INVALID_ID";
const FIRST_PAUSE: Duration = Duration::from_secs(4);
const LONGEST_PAUSE: Duration = Duration::from_secs(64);

#[derive(Debug)]
pub struct Refused {
    pub code: String,
    pub message: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code.as_str() {
            "AUTH_BAD_APIKEY" | "AUTH_MISSING_APIKEY" => {
                write!(f, "AllDebrid rejected the API key ({})", self.code)
            }
            _ => write!(f, "AllDebrid: {} ({})", self.message, self.code),
        }
    }
}

impl std::error::Error for Refused {}

impl Refused {
    pub fn is_auth(&self) -> bool {
        self.code.starts_with("AUTH_")
    }

    pub fn is_capacity(&self) -> bool {
        matches!(
            self.code.as_str(),
            "MAGNET_TOO_MANY_ACTIVE" | "MAGNET_TOO_MANY"
        )
    }
}

pub fn is_unknown_magnet(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<Refused>()
        .is_some_and(|refused| refused.code == UNKNOWN_MAGNET)
}

#[derive(Debug)]
pub struct RetryLater {
    pub status: u16,
    pub wait: Duration,
}

impl fmt::Display for RetryLater {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.status == 0 {
            write!(
                f,
                "AllDebrid asked this module to slow down; retry in {}s",
                self.wait.as_secs()
            )
        } else {
            write!(
                f,
                "AllDebrid answered HTTP {} (rate limit or outage); retry in {}s",
                self.status,
                self.wait.as_secs()
            )
        }
    }
}

impl std::error::Error for RetryLater {}

#[derive(Debug)]
pub struct Delayed {
    pub id: String,
}

impl fmt::Display for Delayed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AllDebrid is still generating this link (delayed link {}); the delayed-link flow is not supported, retry shortly",
            self.id
        )
    }
}

impl std::error::Error for Delayed {}

pub fn is_retry_later(error: &anyhow::Error) -> bool {
    error.is::<RetryLater>() || error.is::<Delayed>()
}

struct Pause {
    until: Instant,
    last: Duration,
}

fn pauses() -> &'static Mutex<HashMap<String, Pause>> {
    static PAUSES: OnceLock<Mutex<HashMap<String, Pause>>> = OnceLock::new();
    PAUSES.get_or_init(Mutex::default)
}

pub(super) fn check(base: &str, now: Instant) -> Result<(), RetryLater> {
    let held = pauses().lock().unwrap_or_else(PoisonError::into_inner);
    match held.get(base) {
        Some(pause) if pause.until > now => Err(RetryLater {
            status: 0,
            wait: pause.until - now,
        }),
        _ => Ok(()),
    }
}

pub(super) fn limited(base: &str, status: u16, now: Instant) -> RetryLater {
    let mut held = pauses().lock().unwrap_or_else(PoisonError::into_inner);
    let last = held
        .get(base)
        .map_or(FIRST_PAUSE / 2, |pause| pause.last)
        .saturating_mul(2)
        .min(LONGEST_PAUSE);
    held.insert(
        base.to_string(),
        Pause {
            until: now + last,
            last,
        },
    );
    RetryLater { status, wait: last }
}

pub(super) fn answered(base: &str) {
    pauses()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(base);
}
