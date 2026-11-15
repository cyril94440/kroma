use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Action, FailoverPolicy, Limits, Observation, Persisted};
use crate::model::ProviderId;
use crate::portal::redact;
use crate::trial::Outcome;

pub const MAX_BYTES: u64 = 2_000_000;
pub const SAMPLE_SECONDS: f64 = 30.0;
const REPLAY_STEP: f64 = 10.0;
const NEAR_SAME_INSTANT: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TraceRecord {
    pub provider: Option<ProviderId>,
    pub paused: bool,
    pub media: bool,
    pub unstable: bool,
    pub viewers: usize,
    pub program: Option<String>,
    pub channel: Option<String>,
    pub action: Option<Action>,
    pub source_kbps: Option<f64>,
    pub kbps_median: Option<f64>,
    pub restarts: Option<u32>,
    pub loss_60s: Option<f64>,
    pub health: Option<String>,
    pub dead_since: Option<f64>,
    pub unstable_since: Option<f64>,
}

type Inputs = (Option<ProviderId>, bool, bool, bool, usize, Option<String>, Option<String>, Option<Action>);

impl TraceRecord {
    fn inputs(&self) -> Inputs {
        (self.provider, self.paused, self.media, self.unstable, self.viewers, self.program.clone(), self.channel.clone(), self.action)
    }

    fn masked(&self) -> TraceRecord {
        TraceRecord { program: self.program.as_deref().map(redact), channel: self.channel.as_deref().map(redact), ..self.clone() }
    }

    fn observation(&self, now: f64) -> Observation {
        Observation { now, provider: self.provider, paused: self.paused, media: self.media, unstable: self.unstable, viewers: self.viewers, program: self.program.clone(), channel: self.channel.clone() }
    }
}

#[derive(Serialize, Deserialize)]
struct Line {
    ts: f64,
    #[serde(flatten)]
    record: TraceRecord,
}

pub struct ProviderTrace {
    path: PathBuf,
    max_bytes: u64,
    sample_seconds: f64,
    last: Option<Inputs>,
    last_at: f64,
}

impl ProviderTrace {
    pub fn new(path: PathBuf) -> ProviderTrace {
        ProviderTrace { path, max_bytes: MAX_BYTES, sample_seconds: SAMPLE_SECONDS, last: None, last_at: 0.0 }
    }

    pub fn with_limits(mut self, max_bytes: u64, sample_seconds: f64) -> ProviderTrace {
        self.max_bytes = max_bytes;
        self.sample_seconds = sample_seconds;
        self
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn record(&mut self, now: f64, record: &TraceRecord, trouble: bool) -> bool {
        let inputs = record.inputs();
        if self.last.as_ref() == Some(&inputs) && !(trouble && now - self.last_at >= self.sample_seconds) {
            return false;
        }
        self.last = Some(inputs);
        self.last_at = now;
        self.append(&Line { ts: (now * 10.0).round() / 10.0, record: record.masked() }).is_ok()
    }

    fn append(&self, line: &Line) -> std::io::Result<()> {
        if let Some(folder) = self.path.parent() {
            std::fs::create_dir_all(folder)?;
        }
        if std::fs::metadata(&self.path).is_ok_and(|meta| meta.len() > self.max_bytes) {
            let mut older = self.path.clone().into_os_string();
            older.push(".1");
            std::fs::rename(&self.path, older)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        let mut text = serde_json::to_string(line).map_err(std::io::Error::other)?;
        text.push('\n');
        file.write_all(text.as_bytes())
    }

    pub fn recent(&self, limit: usize) -> Vec<String> {
        let mut lines: Vec<String> = [self.older_path(), self.path.clone()]
            .iter()
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .flat_map(|text| text.lines().map(str::to_string).collect::<Vec<_>>())
            .collect();
        let skip = lines.len().saturating_sub(limit);
        lines.drain(..skip);
        lines
    }

    fn older_path(&self) -> PathBuf {
        let mut older = self.path.clone().into_os_string();
        older.push(".1");
        PathBuf::from(older)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Replayed {
    pub at: f64,
    pub replayed: Option<Action>,
    pub recorded: Option<Action>,
}

fn replay_step(policy: &mut FailoverPolicy, now: f64, record: &TraceRecord) -> Option<Action> {
    let observation = record.observation(now);
    let action = policy.observe(&observation);
    if let Some(action) = action {
        policy.acknowledge(action, &observation);
        if matches!(action, Action::TrialBackup | Action::TrialPrimary) {
            policy.trial_done(now, Outcome::Impossible, "replay", false, None);
        }
    }
    action
}

pub fn replay<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<Replayed> {
    let records: Vec<Line> = lines.into_iter().filter_map(|text| serde_json::from_str::<Line>(text).ok()).collect();
    let start = records.first().map_or(0.0, |line| line.ts);
    let mut policy = FailoverPolicy::new(true, Limits::default(), Persisted::default(), start);
    let mut out = Vec::new();
    let mut previous: Option<&Line> = None;
    for line in &records {
        let mut at = previous.map_or(line.ts, |prev| prev.ts + REPLAY_STEP);
        while let Some(prev) = previous.filter(|_| at < line.ts - NEAR_SAME_INSTANT) {
            if let Some(action) = replay_step(&mut policy, at, &prev.record) {
                out.push(Replayed { at, replayed: Some(action), recorded: None });
            }
            at += REPLAY_STEP;
        }
        let action = replay_step(&mut policy, line.ts, &line.record);
        if action.is_some() || line.record.action.is_some() {
            out.push(Replayed { at: line.ts, replayed: action, recorded: line.record.action });
        }
        previous = Some(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incident(path: PathBuf) -> ProviderTrace {
        let mut trace = ProviderTrace::new(path);
        let mut policy = FailoverPolicy::new(true, Limits::default(), Persisted::default(), 900.0);
        for step in 0..10 {
            let now = 1000.0 + f64::from(step) * 10.0;
            let dead = now >= 1020.0;
            let record = TraceRecord {
                provider: Some(ProviderId::Primary),
                viewers: 2,
                program: Some("Match".into()),
                channel: Some("VIP: CANAL+ RAW".into()),
                media: !dead,
                source_kbps: Some(if dead { 0.0 } else { 3000.0 }),
                ..TraceRecord::default()
            };
            let action = policy.observe(&record.observation(now));
            if let Some(action) = action {
                policy.acknowledge(action, &record.observation(now));
            }
            trace.record(now, &TraceRecord { action, ..record }, dead);
        }
        trace
    }

    fn scratch() -> kroma_module_sdk::testing::TempDir {
        kroma_module_sdk::testing::temp_dir("live-trace")
    }

    #[test]
    fn a_trace_is_one_line_per_change_and_replays_to_the_recorded_decision() {
        let dir = scratch();
        let trace = incident(dir.path().join("trace.jsonl"));

        let lines = trace.recent(100);
        let replayed = replay(lines.iter().map(String::as_str));

        assert!(lines.len() > 2 && lines.len() < 10, "{}", lines.len());
        assert!(!lines.join("").contains("http"));
        let decided: Vec<_> = replayed.iter().filter(|r| r.replayed.is_some()).map(|r| (r.at, r.replayed.unwrap())).collect();
        assert_eq!(decided, [(1070.0, Action::Failover)]);
        let recorded: Vec<_> = replayed.iter().filter(|r| r.recorded.is_some()).map(|r| (r.at, r.recorded.unwrap())).collect();
        assert_eq!(recorded, [(1070.0, Action::Failover)]);
    }

    #[test]
    fn the_trace_keeps_one_older_generation_and_ignores_unreadable_lines() {
        let dir = scratch();
        let path = dir.path().join("rotating.jsonl");
        let mut trace = ProviderTrace::new(path.clone()).with_limits(100, SAMPLE_SECONDS);

        for viewers in 0..6 {
            trace.record(2000.0 + viewers as f64, &TraceRecord { provider: Some(ProviderId::Primary), media: true, viewers, ..TraceRecord::default() }, false);
        }

        assert!(trace.older_path().exists());
        assert!(std::fs::metadata(&path).unwrap().len() < 400);
        assert!(replay(["not json", "[1]", "{}"]).is_empty());
    }

    #[test]
    fn a_sample_is_written_every_thirty_seconds_only_while_in_trouble() {
        let dir = scratch();
        let mut trace = ProviderTrace::new(dir.path().join("sample.jsonl"));
        let record = TraceRecord { provider: Some(ProviderId::Primary), media: true, viewers: 2, ..TraceRecord::default() };

        assert!(trace.record(0.0, &record, false));
        assert!(!trace.record(10.0, &record, false));
        assert!(!trace.record(100.0, &record, false));
        assert!(trace.record(110.0, &record, true));
        assert!(!trace.record(120.0, &record, true));
    }

    #[test]
    fn urls_in_names_never_reach_the_file() {
        let dir = scratch();
        let mut trace = ProviderTrace::new(dir.path().join("masked.jsonl"));
        let record = TraceRecord { channel: Some("x http://portal.test:80/get.php?username=TESTUSER&password=TESTPASS42 y".into()), ..TraceRecord::default() };

        trace.record(1.0, &record, false);

        let text = trace.recent(10).join("");
        assert!(!text.contains("TESTPASS42") && !text.contains("portal.test"), "{text}");
    }
}
