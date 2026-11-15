use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

use kroma_module_sdk::host::process::Child;

use super::{codec, media, probing, resolution, CoverageState, ScanState};
use crate::clipbuf::Buffer;
use crate::failover::bitrate::BitrateWatch;
use crate::failover::trace::ProviderTrace;
use crate::failover::FailoverPolicy;
use crate::health::StreamHealth;
use crate::model::{Channel, ProviderId};
use crate::presence::Presence;
use crate::probe::Streaks;
use crate::status::{OffAirView, Phase, Proposal};

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
    pub recheck_busy: bool,
    pub last_restart: Option<Instant>,
    pub buffer: Buffer,
    pub last_buffer_purge: Option<Instant>,
    pub last_clip_purge: Option<Instant>,
    pub resolution: String,
    pub resolution_probe: Option<resolution::ResolutionRun>,
    pub last_resolution: Option<Instant>,
    pub codecs: HashMap<(ProviderId, i64), codec::VideoCodec>,
    pub retag: bool,
}

impl Inner {
    pub fn in_trouble(&self, now: f64) -> bool {
        self.policy.trouble(now) || self.crashed_and_silent()
    }

    fn crashed_and_silent(&self) -> bool {
        self.policy.has_backup && self.restarts >= 1 && (self.run.is_none() || self.kbps <= 0.0)
    }

    pub fn new(policy: FailoverPolicy, trace_path: PathBuf) -> Inner {
        Inner {
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
            trace: ProviderTrace::new(trace_path),
            coverage: CoverageState::default(),
            action_busy: false,
            recheck_busy: false,
            last_restart: None,
            buffer: Buffer::default(),
            last_buffer_purge: None,
            last_clip_purge: None,
            resolution: String::new(),
            resolution_probe: None,
            last_resolution: None,
            codecs: HashMap::new(),
            retag: false,
        }
    }
}
