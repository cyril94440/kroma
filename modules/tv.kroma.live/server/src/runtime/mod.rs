mod actions;
mod canary;
mod clips;
mod codec;
mod connections;
mod coverage;
mod emit;
mod frame;
mod guide;
mod media;
mod notify;
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

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use kroma_module_sdk::host::process::Child;
use kroma_module_sdk::host::HostCtx;

use crate::clipbuf::Buffer;
use crate::config::Settings;
use crate::db::{self, Pool};
use crate::failover::bitrate::BitrateWatch;
use crate::failover::trace::ProviderTrace;
use crate::failover::{FailoverPolicy, Limits, Persisted};
use crate::health::StreamHealth;
use crate::model::{Channel, ProviderId};
use crate::presence::Presence;
use crate::probe::Streaks;
use crate::providers::Providers;
use crate::rules::Rules;
use crate::status::{OffAirView, Phase, Proposal};

pub use clips::{ClipCut, ClipError};
pub use connections::Refusal;
pub use coverage::CoverageState;
pub use frame::{Frame, FrameError};
#[cfg(test)]
pub use connections::Use;
pub use guide::{now_next, to_programme};
pub use scan::ScanState;
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
    pub bitrate: BitrateWatch,
    pub trace: ProviderTrace,
    pub coverage: CoverageState,
    pub action_busy: bool,
    pub buffer: Buffer,
    pub last_buffer_purge: Option<Instant>,
    pub last_clip_purge: Option<Instant>,
    pub resolution: String,
    pub resolution_probe: Option<resolution::ResolutionRun>,
    pub last_resolution: Option<Instant>,
    pub codecs: HashMap<(ProviderId, i64), codec::VideoCodec>,
    pub retag: bool,
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
            policy,
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
            bitrate: BitrateWatch::default(),
            trace: ProviderTrace::new(root.join(TRACE_DIR).join(TRACE_FILE)),
            coverage: CoverageState::default(),
            action_busy: false,
            buffer: Buffer::default(),
            last_buffer_purge: None,
            last_clip_purge: None,
            resolution: String::new(),
            resolution_probe: None,
            last_resolution: None,
            codecs: HashMap::new(),
            retag: false,
        };
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
