use std::collections::VecDeque;

mod judge;

pub const GRACE_BLOCKS: usize = 15;
pub const OPENING_WINDOW: usize = 5;
pub const OPENING_THRESHOLD: f64 = 1.0;
const CLOSING_THRESHOLD: f64 = 0.15;
const CLASSIFY_BLOCKS: usize = 20;
pub const CATCH_UP_MARGIN: f64 = 0.5;
const FROZEN_THRESHOLD: f64 = 2.0;
const LOSS_WINDOW: usize = 60;
pub const DEGRADED_LOSS: f64 = 2.0;
pub const SEVERE_LOSS: f64 = 8.0;
const EXIT_LOSS: f64 = 0.6;
const EXIT_BLOCKS: usize = 30;
const MAX_EPISODES: usize = 40;

const SOURCE_HINTS: &[&str] = &[
    "will reconnect",
    "continuity check failed",
    "pes packet size mismatch",
    "corrupt",
    "invalid data found",
    "server returned 4",
    "server returned 5",
];
const LOCAL_HINTS: &[&str] = &["non monoton", "non-monoton", "dts discontinuity"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Starting,
    Healthy,
    Degraded,
    Severe,
    Undetermined,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Starting => "starting",
            State::Healthy => "healthy",
            State::Degraded => "degraded",
            State::Severe => "severe",
            State::Undetermined => "undetermined",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    CaughtUp,
    Lost,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Episode {
    pub at: usize,
    pub class: Class,
    pub magnitude: f64,
    pub freeze: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gap {
    pub opening_lag: f64,
    pub opened_at: usize,
    pub peak: f64,
    pub closing_lag: f64,
    pub closed_at: usize,
    pub lowest_after: f64,
    stable: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Source,
    Local,
    Undetermined,
}

#[derive(Debug, Clone)]
pub struct StreamHealth {
    pub episodes: VecDeque<Episode>,
    pub episodes_total: u64,
    pub channel_loss: f64,
    pub source_hints: u32,
    pub local_hints: u32,
    pub state: State,
    blocks: usize,
    anchor: Option<(usize, f64)>,
    pub lags: VecDeque<f64>,
    pub gap: Option<Gap>,
    last_gap_closed: i64,
    pub pending: Vec<Gap>,
    pub frozen: bool,
    frozen_since: Option<usize>,
    exit_ok: usize,
}

impl Default for StreamHealth {
    fn default() -> Self {
        StreamHealth {
            episodes: VecDeque::new(),
            episodes_total: 0,
            channel_loss: 0.0,
            source_hints: 0,
            local_hints: 0,
            state: State::Starting,
            blocks: 0,
            anchor: None,
            lags: VecDeque::new(),
            gap: None,
            last_gap_closed: -999,
            pending: Vec::new(),
            frozen: false,
            frozen_since: None,
            exit_ok: 0,
        }
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

impl StreamHealth {
    fn reset_anchor(&mut self) {
        self.blocks = 0;
        self.anchor = None;
        self.lags.clear();
        self.gap = None;
        self.last_gap_closed = -999;
        self.pending.clear();
        self.frozen = false;
        self.frozen_since = None;
        self.exit_ok = 0;
    }

    pub fn on_start(&mut self) {
        self.reset_anchor();
        self.state = State::Starting;
    }

    pub fn on_channel_change(&mut self) {
        self.episodes.clear();
        self.channel_loss = 0.0;
        self.source_hints = 0;
        self.local_hints = 0;
        self.on_start();
    }

    pub fn on_pause(&mut self) {
        self.reset_anchor();
        self.state = State::Undetermined;
    }

    pub fn on_stderr(&mut self, line: &str) {
        let lower = line.to_lowercase();
        if SOURCE_HINTS.iter().any(|hint| lower.contains(hint)) {
            self.source_hints += 1;
        } else if LOCAL_HINTS.iter().any(|hint| lower.contains(hint)) {
            self.local_hints += 1;
        }
    }

    pub fn on_block(&mut self, out_time: f64) {
        self.blocks += 1;
        let Some((anchor_block, anchor_time)) = self.anchor else {
            if self.blocks >= GRACE_BLOCKS {
                self.anchor = Some((self.blocks, out_time));
                self.state = State::Healthy;
            }
            return;
        };
        let lag = (self.blocks - anchor_block) as f64 - (out_time - anchor_time);
        if self.lags.len() >= OPENING_WINDOW + CLASSIFY_BLOCKS + 5 {
            self.lags.pop_front();
        }
        self.lags.push_back(lag);
        self.update_gap(lag);
        self.classify_pending(lag);
        self.update_frozen(lag);
        self.update_state();
    }

    fn lag_ago(&self, steps: usize) -> Option<f64> {
        self.lags.len().checked_sub(steps + 1).map(|at| self.lags[at])
    }

    fn update_gap(&mut self, lag: f64) {
        let Some(gap) = self.gap.as_mut() else {
            if (self.blocks as i64) - self.last_gap_closed < OPENING_WINDOW as i64 {
                return;
            }
            if let Some(reference) = self.lag_ago(OPENING_WINDOW) {
                if lag - reference >= OPENING_THRESHOLD {
                    self.gap = Some(Gap {
                        opening_lag: reference,
                        opened_at: self.blocks,
                        peak: lag,
                        closing_lag: 0.0,
                        closed_at: 0,
                        lowest_after: 0.0,
                        stable: 0,
                    });
                }
            }
            return;
        };
        gap.peak = gap.peak.max(lag);
        let reference = self.lags.len().checked_sub(3).map(|at| self.lags[at]);
        match reference {
            Some(before) if (lag - before).abs() <= CLOSING_THRESHOLD => {
                gap.stable += 1;
                if gap.stable >= 2 {
                    let mut closed = *gap;
                    closed.closing_lag = lag;
                    closed.closed_at = self.blocks;
                    closed.lowest_after = lag;
                    self.pending.push(closed);
                    self.gap = None;
                    self.last_gap_closed = self.blocks as i64;
                }
            }
            _ => gap.stable = 0,
        }
    }
}

#[cfg(test)]
impl StreamHealth {
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    pub fn is_armed(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn current_lag(&self) -> Option<f64> {
        self.lags.back().copied()
    }

    pub fn caught_up_gaps(&self) -> usize {
        self.episodes.iter().filter(|e| e.class == Class::CaughtUp).count()
    }
}
