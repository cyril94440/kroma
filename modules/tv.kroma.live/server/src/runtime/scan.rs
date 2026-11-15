use std::sync::Arc;
use std::time::Duration;

use kroma_module_sdk::host::process::Command;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::connections::Use;
use super::process::{drained, stop_child, wait_for_exit};
use super::snapshot::TOPIC_SCAN;
use super::Live;
use crate::catalogue::SlotCandidate;
use crate::model::ProviderId;
use crate::scanframe::{capture_args, pick_frames, black_in_report, CAPTURE_SECONDS};

pub const CAPTURE_LIMIT: Duration = Duration::from_secs(25);
pub const BENCHMARK_MAX_IDS: usize = 8;
pub const REPEAT_WITHIN: Duration = Duration::from_secs(60);
const MAX_CANDIDATES: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanItem {
    pub number: Option<u32>,
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub black: bool,
    pub still: bool,
    pub frame: Option<String>,
    pub frames: Vec<String>,
    pub error: Option<String>,
    pub now: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanState {
    pub id: String,
    pub query: String,
    pub running: bool,
    pub provider: Option<ProviderId>,
    pub items: Vec<ScanItem>,
    pub total: usize,
    #[serde(skip)]
    pub finished: Option<std::time::Instant>,
}

impl Live {
    pub fn scan_state(&self) -> ScanState {
        self.lock().scan.clone()
    }

    pub fn start_scan(self: &Arc<Self>, query: &str, provider: ProviderId) -> Result<String, String> {
        if self.lock().scan.running {
            return Err("scanRunning".into());
        }
        let rules = self.rules();
        let candidates = self.providers.catalogue(provider).slot_candidates(query, MAX_CANDIDATES, None, &rules);
        if candidates.is_empty() {
            return Err("noCandidate".into());
        }
        Ok(self.launch_scan(query, provider, candidates))
    }

    pub fn start_benchmark(self: &Arc<Self>, ids: &[String]) -> Result<(String, bool), String> {
        let mut channels = Vec::new();
        let mut provider = None;
        for id in ids.iter().take(BENCHMARK_MAX_IDS) {
            let channel = self.providers.find_by_ref(id).ok_or("unknownChannel")?;
            if provider.is_some_and(|p| p != channel.provider) {
                return Err("mixedProviders".into());
            }
            provider = Some(channel.provider);
            channels.push(channel);
        }
        let provider = provider.ok_or("noChannel")?;
        let key = ids.join(",");
        {
            let inner = self.lock();
            if inner.scan.running {
                return Err("scanRunning".into());
            }
            if inner.scan.query == key && inner.scan.finished.is_some_and(|at| at.elapsed() < REPEAT_WITHIN) {
                return Ok((inner.scan.id.clone(), true));
            }
        }
        let candidates = channels
            .into_iter()
            .map(|channel| SlotCandidate { family: crate::norm::family_slot(&crate::norm::fold(&channel.name)).0, number: channel.slot.1, channel, alternatives: Vec::new() })
            .collect();
        Ok((self.launch_scan(&key, provider, candidates), false))
    }

    fn launch_scan(self: &Arc<Self>, query: &str, provider: ProviderId, candidates: Vec<SlotCandidate>) -> String {
        let token = kroma_module_sdk::primitives::random_token();
        self.clear_scans();
        self.lock().scan = ScanState { id: token.clone(), query: query.into(), running: true, provider: Some(provider), items: Vec::new(), total: candidates.len(), finished: None };
        let live = self.clone();
        let scan_id = token.clone();
        tokio::spawn(async move {
            for candidate in candidates {
                let mut item = live.capture(&candidate.channel, &scan_id, candidate.number).await;
                if item.error.is_some() {
                    for alternative in &candidate.alternatives {
                        let retry = live.capture(alternative, &scan_id, candidate.number).await;
                        if retry.error.is_none() {
                            item = retry;
                            break;
                        }
                    }
                }
                let (now, _) = super::now_next(&live.pool, candidate.channel.epg_id.as_deref(), &candidate.channel.name, super::now_ms());
                item.now = now.map(|p| p.title);
                live.emit(TOPIC_SCAN, json!({ "id": scan_id, "item": item }));
                live.lock().scan.items.push(item);
            }
            {
                let mut inner = live.lock();
                inner.scan.running = false;
                inner.scan.finished = Some(std::time::Instant::now());
            }
            live.emit(TOPIC_SCAN, json!({ "id": scan_id, "done": true }));
        });
        token
    }

    fn clear_scans(&self) {
        let Ok(entries) = std::fs::read_dir(self.public_dir()) else { return };
        for entry in entries.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("scan-")) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }

    pub(crate) async fn capture(&self, channel: &crate::model::Channel, scan_id: &str, number: Option<u32>) -> ScanItem {
        let mut item = ScanItem { number, id: channel.id(), name: channel.name.clone(), ..ScanItem::default() };
        if let Err(refusal) = self.admission(Use::Capture, channel.provider) {
            item.error = Some(refusal.as_str().into());
            return item;
        }
        let source = match self.source_for(channel) {
            Ok(source) => source,
            Err(error) => {
                item.error = Some(error.to_string());
                return item;
            }
        };
        let folder = self.public_dir().join(format!("scan-{scan_id}"));
        let work = folder.join(format!("work-{}", channel.stream_id));
        let _ = std::fs::create_dir_all(&work);
        let args = capture_args(&source.url, source.user_agent.as_deref().unwrap_or(""), &work.to_string_lossy());
        self.lock().tests_running += 1;
        let spawned = Command::new(self.programs.ffmpeg.clone()).args(args).spawn();
        let report = match spawned {
            Ok(child) => {
                if wait_for_exit(&child, CAPTURE_LIMIT).await.is_none() {
                    stop_child(&child).await;
                }
                drained(&child).1
            }
            Err(error) => {
                item.error = Some(error.to_string());
                String::new()
            }
        };
        {
            let mut inner = self.lock();
            inner.tests_running = inner.tests_running.saturating_sub(1);
        }
        let mut shots: Vec<std::path::PathBuf> = std::fs::read_dir(&work).into_iter().flatten().flatten().map(|e| e.path()).collect();
        shots.sort();
        let blobs: Vec<Vec<u8>> = shots.iter().filter_map(|path| std::fs::read(path).ok()).collect();
        item.black = black_in_report(&report, CAPTURE_SECONDS);
        let picks = pick_frames(&blobs);
        item.ok = !picks.is_empty();
        item.still = picks.len() == 1 && blobs.len() > 1;
        let last = picks.len();
        for (n, index) in picks.into_iter().enumerate() {
            let name = if n + 1 == last { format!("{}.jpg", channel.stream_id) } else { format!("{}-first.jpg", channel.stream_id) };
            if std::fs::write(folder.join(&name), &blobs[index]).is_ok() {
                item.frames.push(format!("scan-{scan_id}/{name}"));
            }
        }
        item.frame = item.frames.last().cloned();
        let _ = std::fs::remove_dir_all(&work);
        if !item.ok && item.error.is_none() {
            item.error = Some("noPicture".into());
        }
        item
    }
}
