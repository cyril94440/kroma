use super::event::{SportEvent, State};
use super::memory::Memory;
use super::reading::Certainty;

pub const SCAN_GAP_S: f64 = 120.0;
pub const SCAN_GAP_FORCED_S: f64 = 20.0;
const IDLE_S: f64 = 900.0;
const LIVE_S: f64 = 45.0;
const NEAR_KICKOFF_S: f64 = 90.0;
const QUIET_S: f64 = 300.0;
const KICKOFF_WINDOW_S: f64 = 1800.0;
const MAX_REFS: usize = 8;

pub fn cadence_seconds(events: &[SportEvent], watched: bool, now: f64) -> f64 {
    if !watched {
        return IDLE_S;
    }
    if events.iter().any(|e| e.state == State::In) {
        return LIVE_S;
    }
    if events
        .iter()
        .any(|e| e.state == State::Pre && (0.0..=KICKOFF_WINDOW_S).contains(&(e.start - now)))
    {
        return NEAR_KICKOFF_S;
    }
    QUIET_S
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScanNeed {
    pub multiplex: bool,
    pub refs: Vec<String>,
    pub reason: &'static str,
}

pub fn need_scan(
    events: &[SportEvent],
    memory: &Memory,
    now: f64,
    force: bool,
    last_scan: f64,
    guide_refs: &[String],
) -> ScanNeed {
    let gap = if force { SCAN_GAP_FORCED_S } else { SCAN_GAP_S };
    if now - last_scan < gap {
        return ScanNeed {
            multiplex: false,
            refs: Vec::new(),
            reason: "vient_de_scanner",
        };
    }
    let live: Vec<&SportEvent> = events
        .iter()
        .filter(|e| e.is_ligue1() && e.state == State::In)
        .collect();
    let missing = live
        .iter()
        .any(|e| memory.seen_channel(&e.id).and_then(|s| s.certainty) != Some(Certainty::Seen));
    let multiplex = !live.is_empty() && (force || missing);
    let refs: Vec<String> = guide_refs.iter().take(MAX_REFS).cloned().collect();
    let reason = if force {
        "force"
    } else if multiplex || !refs.is_empty() {
        "manque"
    } else {
        "tout_identifie"
    };
    ScanNeed {
        multiplex,
        refs,
        reason,
    }
}
