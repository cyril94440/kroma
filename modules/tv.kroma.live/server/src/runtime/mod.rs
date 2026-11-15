mod actions;
mod canary;
mod clips;
mod codec;
mod connections;
mod coverage;
mod emit;
mod files;
mod frame;
mod guide;
mod inner;
mod media;
mod notify;
mod ops;
mod outage;
mod probing;
mod process;
mod quick;
mod resolution;
mod scan;
mod snapshot;
mod switch;
mod tick;
mod trials;
mod watch;

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use kroma_module_sdk::host::HostCtx;

use crate::config::Settings;
use crate::db::{self, Pool};
use crate::failover::{FailoverPolicy, Limits, Persisted};
use crate::model::Channel;
use crate::providers::Providers;
use crate::rules::Rules;
use crate::status::Phase;

#[cfg(test)]
pub use clips::ClipError;
pub use connections::Refusal;
pub use coverage::CoverageState;
#[cfg(test)]
pub use frame::FrameError;
#[cfg(test)]
pub use frame::SETTLE_FOR_TESTS;
#[cfg(test)]
pub use connections::Use;
pub use guide::{now_next, to_programme};
pub use inner::{Inner, Run};
#[cfg(test)]
pub use actions::Proposed;
#[cfg(test)]
pub use codec::VideoCodec;
pub use ops::{ProbeReport, ProviderSwitch, Restart};
#[cfg(test)]
pub use media::READ_BUDGET_BYTES;
pub use scan::{ScanState, BENCHMARK_MAX_IDS};
pub use switch::{SwitchOutcome, SwitchRequest};

pub const KV_CURRENT: &str = "current";
pub const KV_POLICY: &str = "policy";
pub const PUBLIC_DIR: &str = "public";
pub const TRACE_DIR: &str = "trace";
pub const TRACE_FILE: &str = "provider-trace.jsonl";

#[derive(Debug, Clone)]
pub struct Programs {
    pub ffmpeg: String,
    pub ffprobe: String,
}

impl Default for Programs {
    fn default() -> Self {
        Programs { ffmpeg: "ffmpeg".into(), ffprobe: "ffprobe".into() }
    }
}

pub struct Live {
    pub(crate) host: Arc<dyn HostCtx>,
    pub(crate) pool: Pool,
    pub(crate) root: PathBuf,
    pub(crate) programs: Programs,
    pub providers: Providers,
    pub(crate) inner: Mutex<Inner>,
    pub(crate) gate: tokio::sync::Mutex<()>,
    pub(crate) frame: tokio::sync::Mutex<Option<frame::CachedFrame>>,
    #[cfg(test)]
    pub(crate) sent: Mutex<Vec<(String, serde_json::Value)>>,
}

impl Live {
    pub fn new(host: Arc<dyn HostCtx>, pool: Pool, root: PathBuf, programs: Programs) -> Arc<Live> {
        let settings = Settings::read(host.as_ref());
        let now = now_seconds();
        let (saved, unreadable) = db::kv_get(&pool, KV_POLICY).ok().flatten().map(|text| Persisted::from_json(&text)).unwrap_or_default();
        let policy = FailoverPolicy::new(settings.has_backup(), limits_of(&settings), saved, now);
        let ignored: Vec<&str> = unreadable.into_iter().chain(policy.ignored_state.iter().copied()).collect();
        if !ignored.is_empty() {
            tracing::warn!(ignored = ?ignored, "failover state fields ignored at load");
        }
        let inner = Inner::new(policy, root.join(TRACE_DIR).join(TRACE_FILE));
        Arc::new(Live {
            host,
            providers: Providers::new(pool.clone()),
            pool,
            root,
            programs,
            inner: Mutex::new(inner),
            gate: tokio::sync::Mutex::new(()),
            frame: tokio::sync::Mutex::new(None),
            #[cfg(test)]
            sent: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn settings(&self) -> Settings {
        Settings::read(self.host.as_ref())
    }

    pub fn rules(&self) -> Rules {
        Rules::read(self.host.as_ref())
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn public_dir(&self) -> PathBuf {
        self.root.join(PUBLIC_DIR)
    }

    pub fn party_present(&self) -> bool {
        !self.host.contributions("tv.kroma.party/room").is_empty()
    }

    pub fn viewer_count(&self) -> usize {
        self.lock().presence.count()
    }

    pub fn current(&self) -> Option<Channel> {
        self.lock().current.clone()
    }

    pub fn is_playing(&self) -> bool {
        let inner = self.lock();
        inner.run.is_some() && inner.phase != Phase::Paused
    }
}

pub fn limits_of(settings: &Settings) -> Limits {
    Limits {
        dead_after: settings.dead_after,
        unstable_after: settings.unstable_after,
        unstable_window: settings.unstable_window,
        trial_after: settings.trial_after,
        trouble_dead: settings.trouble_dead,
    }
}

pub fn now_ms() -> i64 {
    kroma_module_sdk::primitives::now_ms()
}

pub fn now_seconds() -> f64 {
    now_ms() as f64 / 1000.0
}
