use std::sync::Mutex;

use anyhow::{anyhow, Result};
use serde_json::Value;

use super::{
    Benchmarked, BufferInfo, Candidate, CutClip, Live, LiveStatus, Programme, Room, ScanReport,
    SwitchOutcome,
};

#[derive(Default)]
pub struct FakeLive {
    pub status: Mutex<LiveStatus>,
    pub programmes: Mutex<Vec<Programme>>,
    pub channels: Mutex<Vec<Candidate>>,
    pub buffer: Mutex<Option<BufferInfo>>,
    pub cuts: Mutex<Vec<(i64, i64, String)>>,
    pub cut_supported: Mutex<bool>,
    pub deleted: Mutex<Vec<String>>,
    pub listed: Mutex<Option<Vec<String>>>,
    pub scan: Mutex<Option<ScanReport>>,
    pub benchmarks: Mutex<Vec<Vec<String>>>,
    pub benchmark_status: Mutex<String>,
}

impl Live for FakeLive {
    fn status(&self) -> Result<LiveStatus> {
        Ok(self.status.lock().unwrap().clone())
    }

    fn epg(&self, _query: &str, _from_ms: i64, _to_ms: i64, _limit: u32) -> Result<Vec<Programme>> {
        Ok(self.programmes.lock().unwrap().clone())
    }

    fn resolve(&self, query: &str, limit: u32) -> Result<Vec<Candidate>> {
        let wanted = query.to_lowercase();
        let all = self.channels.lock().unwrap().clone();
        Ok(all
            .into_iter()
            .filter(|c| c.name.to_lowercase().contains(&wanted) || c.id == query)
            .take(limit as usize)
            .collect())
    }

    fn clip_buffer_info(&self) -> Result<Option<BufferInfo>> {
        Ok(self.buffer.lock().unwrap().clone())
    }

    fn clip_cut(&self, from_ms: i64, to_ms: i64, name: &str) -> Result<Option<CutClip>> {
        if !*self.cut_supported.lock().unwrap() {
            return Ok(None);
        }
        self.cuts
            .lock()
            .unwrap()
            .push((from_ms, to_ms, name.to_string()));
        Ok(Some(CutClip {
            path: format!("clips/{name}.mp4"),
            duration_ms: to_ms - from_ms,
        }))
    }

    fn clip_delete(&self, path: &str) -> Result<bool> {
        self.deleted.lock().unwrap().push(path.to_string());
        Ok(true)
    }

    fn clip_list(&self) -> Result<Option<Vec<String>>> {
        Ok(self.listed.lock().unwrap().clone())
    }

    fn scan_result(&self) -> Result<Option<ScanReport>> {
        Ok(self.scan.lock().unwrap().clone())
    }

    fn benchmark(
        &self,
        channel_ids: &[String],
        _requested_by: &str,
    ) -> Result<Option<Benchmarked>> {
        self.benchmarks.lock().unwrap().push(channel_ids.to_vec());
        let status = self.benchmark_status.lock().unwrap().clone();
        Ok(Some(Benchmarked {
            status: if status.is_empty() {
                "started".into()
            } else {
                status
            },
            reason: None,
            already_done: false,
        }))
    }
}

#[derive(Default)]
pub struct FakeRoom {
    pub said: Mutex<Vec<(String, Option<Value>)>>,
    pub switches: Mutex<Vec<(String, String, String)>>,
    pub fail: Mutex<bool>,
}

impl Room for FakeRoom {
    fn say(&self, text: &str, card: Option<Value>) -> Result<String> {
        if *self.fail.lock().unwrap() {
            return Err(anyhow!("no module contributes the room"));
        }
        self.said.lock().unwrap().push((text.to_string(), card));
        Ok(format!("m{}", self.said.lock().unwrap().len()))
    }

    fn request_switch(
        &self,
        channel_id: &str,
        requested_by: &str,
        reason: &str,
    ) -> Result<SwitchOutcome> {
        self.switches.lock().unwrap().push((
            channel_id.to_string(),
            requested_by.to_string(),
            reason.to_string(),
        ));
        Ok(SwitchOutcome {
            outcome: "voteStarted".into(),
            vote_id: Some("1".into()),
            reason: None,
        })
    }
}

impl FakeRoom {
    pub fn texts(&self) -> Vec<String> {
        self.said
            .lock()
            .unwrap()
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }
}
