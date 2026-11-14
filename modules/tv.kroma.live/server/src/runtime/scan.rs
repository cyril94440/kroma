use std::sync::Arc;
use std::time::Duration;

use kroma_module_sdk::host::process::Command;
use kroma_module_sdk::host::Event;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::connections::Use;
use super::process::{drained, stop_child, wait_for_exit};
use super::snapshot::TOPIC_SCAN;
use super::Live;
use crate::model::ProviderId;
use crate::probe::{analyse, SAMPLE_SECONDS};

pub const CAPTURE_LIMIT: Duration = Duration::from_secs(25);
const MAX_CANDIDATES: usize = 12;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ScanItem {
    pub number: Option<u32>,
    pub id: String,
    pub name: String,
    pub ok: bool,
    pub black: bool,
    pub frame: Option<String>,
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
        let token = kroma_module_sdk::primitives::random_token();
        self.clear_scans();
        self.lock().scan = ScanState { id: token.clone(), query: query.into(), running: true, provider: Some(provider), items: Vec::new(), total: candidates.len() };
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
                live.host.publish(Event::new(TOPIC_SCAN, json!({ "id": scan_id, "item": item })));
                live.lock().scan.items.push(item);
            }
            live.lock().scan.running = false;
            live.host.publish(Event::new(TOPIC_SCAN, json!({ "id": scan_id, "done": true })));
        });
        Ok(token)
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
        let _ = std::fs::create_dir_all(&folder);
        let file = format!("{}.jpg", folder.join(channel.stream_id.to_string()).to_string_lossy());
        let args: Vec<String> = [
            "-hide_banner", "-loglevel", "info", "-nostdin", "-user_agent", source.user_agent.as_deref().unwrap_or(""), "-rw_timeout", "15000000", "-analyzeduration", "3000000", "-probesize", "5000000", "-skip_frame", "nokey", "-i", &source.url, "-t", &SAMPLE_SECONDS.to_string(), "-an", "-vf", "blackdetect=d=2.4:pix_th=0.10,scale=320:-2", "-update", "1", "-f", "image2", "-y", &file,
        ]
        .map(String::from)
        .to_vec();
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
        let exists = std::path::Path::new(&file).exists();
        item.ok = exists;
        if exists {
            item.frame = Some(format!("scan-{scan_id}/{}.jpg", channel.stream_id));
            item.black = analyse(&report, SAMPLE_SECONDS, 1, true).is_some_and(|v| v.black);
        } else if item.error.is_none() {
            item.error = Some("noPicture".into());
        }
        item
    }
}
