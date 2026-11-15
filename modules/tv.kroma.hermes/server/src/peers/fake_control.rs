use std::sync::Mutex;

use anyhow::{anyhow, Result};

use super::fakes::answer;
use super::ops::{History, Restart};
use super::{BenchmarkStart, Candidate, Control, Probe, Programme, ProviderSwitch, ScanState, Snapshot, Status};

pub type Reply<T> = Mutex<Option<Result<T, String>>>;

#[derive(Default)]
pub struct FakeControl {
    pub status: Mutex<Status>,
    pub channels: Mutex<Vec<Candidate>>,
    pub programmes: Mutex<Vec<Programme>>,
    pub snapshot: Reply<Snapshot>,
    pub restart: Reply<Restart>,
    pub provider: Reply<ProviderSwitch>,
    pub channel_provider: Reply<ProviderSwitch>,
    pub probe: Reply<Probe>,
    pub benchmark: Reply<BenchmarkStart>,
    pub scans: Mutex<Vec<ScanState>>,
    pub history: Reply<History>,
    pub frames: Mutex<Option<Result<std::collections::BTreeMap<String, String>, String>>>,
    pub down: Mutex<bool>,
    pub status_after_snapshot: Mutex<Option<Status>>,
    pub epg_calls: Mutex<Vec<Option<String>>>,
    pub resolve_calls: Mutex<Vec<(String, Option<String>)>>,
    pub calls: Mutex<Vec<(String, String)>>,
    pub probe_ids: Mutex<Vec<String>>,
    pub benchmark_ids: Mutex<Vec<Vec<String>>>,
    pub history_limits: Mutex<Vec<u32>>,
    pub provider_asks: Mutex<Vec<(String, Option<String>, String)>>,
}

impl FakeControl {
    fn note(&self, method: &str, who: &str) {
        self.calls.lock().unwrap().push((method.into(), who.into()));
    }
}

impl Control for FakeControl {
    fn status(&self) -> Result<Status> {
        if *self.down.lock().unwrap() {
            return Err(anyhow!("live is not running"));
        }
        Ok(self.status.lock().unwrap().clone())
    }

    fn resolve(&self, query: &str, limit: u32, provider: Option<&str>) -> Result<Vec<Candidate>> {
        self.resolve_calls.lock().unwrap().push((query.into(), provider.map(str::to_string)));
        let q = query.to_lowercase();
        let channels = self.channels.lock().unwrap();
        let mut found: Vec<Candidate> = channels
            .iter()
            .filter(|c| c.id == query || c.name.to_lowercase().contains(&q))
            .filter(|c| provider.is_none_or(|p| c.provider.is_empty() || c.provider == p))
            .cloned()
            .collect();
        found.truncate(limit as usize);
        Ok(found)
    }

    fn epg(&self, query: Option<&str>, from: Option<i64>, to: Option<i64>, limit: u32) -> Result<Vec<Programme>> {
        self.epg_calls.lock().unwrap().push(query.map(str::to_string));
        let q = query.unwrap_or_default().to_lowercase();
        let words: Vec<&str> = q.split_whitespace().collect();
        let mut found: Vec<Programme> = self
            .programmes
            .lock()
            .unwrap()
            .iter()
            .filter(|p| {
                let hay = format!("{} {}", p.title, p.description).to_lowercase();
                words.iter().any(|w| hay.contains(w)) && from.is_none_or(|f| p.end > f) && to.is_none_or(|t| p.start < t)
            })
            .cloned()
            .collect();
        found.truncate(limit as usize);
        Ok(found)
    }

    fn snapshot(&self) -> Result<Snapshot> {
        let taken = answer(&self.snapshot);
        if let Some(next) = self.status_after_snapshot.lock().unwrap().take() {
            *self.status.lock().unwrap() = next;
        }
        taken
    }

    fn restart(&self, requested_by: &str, _reason: &str) -> Result<Restart> {
        self.note("restart", requested_by);
        answer(&self.restart)
    }

    fn switch_provider(&self, provider: &str, channel_id: Option<&str>, requested_by: &str, reason: &str) -> Result<ProviderSwitch> {
        self.note("switchProvider", &format!("{provider}:{requested_by}"));
        self.provider_asks.lock().unwrap().push((provider.into(), channel_id.map(str::to_string), reason.into()));
        let scripted = self.channel_provider.lock().unwrap().is_some();
        if channel_id.is_some() && scripted {
            answer(&self.channel_provider)
        } else {
            answer(&self.provider)
        }
    }

    fn probe(&self, channel_id: &str) -> Result<Probe> {
        self.probe_ids.lock().unwrap().push(channel_id.into());
        answer(&self.probe)
    }

    fn benchmark(&self, channel_ids: &[String], requested_by: &str) -> Result<BenchmarkStart> {
        self.note("benchmark", requested_by);
        self.benchmark_ids.lock().unwrap().push(channel_ids.to_vec());
        answer(&self.benchmark)
    }

    fn scan_result(&self) -> Result<ScanState> {
        let mut scans = self.scans.lock().unwrap();
        match scans.len() {
            0 => Err(anyhow!("HTTP 404: not found")),
            1 => Ok(scans[0].clone()),
            _ => Ok(scans.remove(0)),
        }
    }

    fn history(&self, limit: u32) -> Result<History> {
        self.history_limits.lock().unwrap().push(limit);
        answer(&self.history)
    }

    fn scan_frame(&self, path: &str) -> Result<String> {
        match self.frames.lock().unwrap().clone() {
            Some(Ok(map)) => map.get(path).cloned().ok_or_else(|| anyhow!("unknownPath")),
            Some(Err(e)) => Err(anyhow!(e)),
            None => Err(anyhow!("HTTP 404: not found")),
        }
    }
}
