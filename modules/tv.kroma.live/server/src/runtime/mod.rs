mod actions;
mod connections;
mod guide;
mod media;
mod process;
mod probing;
mod scan;
mod snapshot;
mod switch;
mod tick;
mod trials;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use kroma_module_sdk::host::process::Child;
use kroma_module_sdk::host::HostCtx;

use crate::config::Settings;
use crate::db::{self, Pool};
use crate::failover::{FailoverPolicy, Limits};
use crate::health::StreamHealth;
use crate::model::Channel;
use crate::presence::Presence;
use crate::probe::Streaks;
use crate::providers::Providers;
use crate::rules::Rules;
use crate::status::{OffAirView, Phase, Proposal};

pub use connections::Refusal;
#[cfg(test)]
pub use connections::Use;
pub use guide::{now_next, to_programme};
pub use scan::ScanState;
pub use switch::{SwitchOutcome, SwitchRequest};

pub const KV_CURRENT: &str = "current";
pub const KV_POLICY: &str = "policy";
pub const PUBLIC_DIR: &str = "public";

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

pub struct Run {
    pub child: Child,
    pub folder: String,
    pub carry: String,
}

pub struct Inner {
    pub phase: Phase,
    pub current: Option<Channel>,
    pub run: Option<Run>,
    pub generation: u64,
    pub restarts: u32,
    pub restarts_total: u64,
    pub restart_at: Option<Instant>,
    pub started_at: Option<Instant>,
    pub started_wall: Option<i64>,
    pub health: StreamHealth,
    pub streaks: Streaks,
    pub black: bool,
    pub frozen: bool,
    pub kbps: f64,
    pub ready: bool,
    pub last_segment_at: Option<Instant>,
    pub stderr_tail: VecDeque<String>,
    pub last_error: Option<String>,
    pub presence: Presence,
    pub policy: FailoverPolicy,
    pub probe: Option<probing::ProbeRun>,
    pub last_probe: Option<Instant>,
    pub last_observe: Option<Instant>,
    pub last_ping: Option<Instant>,
    pub last_history: Option<Instant>,
    pub last_media_check: Option<Instant>,
    pub off_air: Option<OffAirView>,
    pub proposals: HashMap<String, Proposal>,
    pub tests_running: usize,
    pub arriving: bool,
    pub trial_cancel: bool,
    pub scan: ScanState,
    pub zap: Option<media::ZapTiming>,
    pub last_status_json: String,
    pub last_health_json: String,
    pub last_health_at: Option<Instant>,
    pub saved_policy_json: String,
    pub https_ok: Option<bool>,
}

pub struct Live {
    pub(crate) host: Arc<dyn HostCtx>,
    pub(crate) pool: Pool,
    pub(crate) root: PathBuf,
    pub(crate) programs: Programs,
    pub providers: Providers,
    pub(crate) inner: Mutex<Inner>,
    pub(crate) gate: tokio::sync::Mutex<()>,
}

impl Live {
    pub fn new(host: Arc<dyn HostCtx>, pool: Pool, root: PathBuf, programs: Programs) -> Arc<Live> {
        let settings = Settings::read(host.as_ref());
        let saved = db::kv_get_json(&pool, KV_POLICY).unwrap_or_default();
        let now = now_seconds();
        let inner = Inner {
            phase: Phase::Paused,
            current: None,
            run: None,
            generation: 0,
            restarts: 0,
            restarts_total: 0,
            restart_at: None,
            started_at: None,
            started_wall: None,
            health: StreamHealth::default(),
            streaks: Streaks::default(),
            black: false,
            frozen: false,
            kbps: 0.0,
            ready: false,
            last_segment_at: None,
            stderr_tail: VecDeque::new(),
            last_error: None,
            presence: Presence::default(),
            policy: FailoverPolicy::new(settings.has_backup(), limits_of(&settings), saved, now),
            probe: None,
            last_probe: None,
            last_observe: None,
            last_ping: None,
            last_history: None,
            last_media_check: None,
            off_air: None,
            proposals: HashMap::new(),
            tests_running: 0,
            arriving: false,
            trial_cancel: false,
            scan: ScanState::default(),
            zap: None,
            last_status_json: String::new(),
            last_health_json: String::new(),
            last_health_at: None,
            saved_policy_json: String::new(),
            https_ok: None,
        };
        Arc::new(Live {
            host,
            providers: Providers::new(pool.clone()),
            pool,
            root,
            programs,
            inner: Mutex::new(inner),
            gate: tokio::sync::Mutex::new(()),
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

    pub fn is_viewing(&self, user_id: &str) -> bool {
        self.lock().presence.viewers().iter().any(|v| v.user_id == user_id)
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
