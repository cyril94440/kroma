use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use kroma_module_sdk::host::process::{Child, Command};

use super::{Live, Run, KV_CURRENT};
use crate::cmd::{self, Source};
use crate::db;
use crate::model::Channel;
use crate::portal::redact;
use crate::status::Phase;

pub const STOP_GRACE: Duration = Duration::from_secs(3);
const POLL: Duration = Duration::from_millis(50);

#[derive(serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedCurrent {
    pub reference: String,
    pub paused: bool,
}

impl Live {
    pub(crate) fn ffmpeg_supports_https(&self) -> bool {
        if let Some(known) = self.lock().https_ok {
            return known;
        }
        let supported = Command::new(self.programs.ffmpeg.clone())
            .args(["-hide_banner", "-protocols"])
            .output(Duration::from_secs(5))
            .map(|out| {
                let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
                let inputs = text.split("output:").next().unwrap_or("");
                inputs.lines().any(|l| l.trim() == "https") && inputs.lines().any(|l| l.trim() == "tls")
            })
            .unwrap_or(false);
        self.lock().https_ok = Some(supported);
        supported
    }

    pub(crate) fn remember_current(&self, paused: bool) {
        let reference = self.lock().current.as_ref().map(|c| c.id()).unwrap_or_default();
        let _ = db::kv_set_json(&self.pool, KV_CURRENT, &SavedCurrent { reference, paused });
    }

    pub(crate) fn folder_path(&self, folder: &str) -> PathBuf {
        self.public_dir().join(folder)
    }

    fn fresh_folder(&self) -> Result<String> {
        let name = kroma_module_sdk::primitives::random_token();
        std::fs::create_dir_all(self.public_dir().join(&name))?;
        Ok(name)
    }

    pub(crate) fn clear_public_except(&self, keep: Option<&str>) {
        let Ok(entries) = std::fs::read_dir(self.public_dir()) else { return };
        for entry in entries.flatten() {
            if keep.is_some_and(|k| entry.file_name() == k) {
                continue;
            }
            if entry.file_name() == crate::clipbuf::CLIPS_DIR || (keep.is_some() && entry.file_name().to_string_lossy().starts_with("scan-")) {
                continue;
            }
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }

    pub(crate) fn source_for(&self, channel: &Channel) -> Result<Source> {
        let settings = self.settings();
        let url = self.providers.stream_url(channel, &settings).ok_or_else(|| anyhow!("the {} provider is not configured", channel.provider.as_str()))?;
        Ok(Source {
            name: channel.name.clone(),
            url,
            provider: Some(channel.provider),
            user_agent: Some(settings.user_agent_of(channel.provider).to_string()),
            realtime: false,
            audio: None,
            hevc: self.is_hevc(channel),
        })
    }

    pub(crate) fn start_run(&self, channel: &Channel, relaunch: bool) -> Result<()> {
        let source = self.source_for(channel)?;
        if source.url.starts_with("https") && !self.ffmpeg_supports_https() {
            bail!("this server's ffmpeg has no https support: use an http provider URL or install an ffmpeg with TLS");
        }
        let settings = self.settings();
        let folder = self.fresh_folder()?;
        let path = self.folder_path(&folder);
        let args = cmd::build(&source, &settings.cmd_options(), &path.to_string_lossy());
        let child = Command::new(self.programs.ffmpeg.clone()).args(args).spawn().map_err(|e| anyhow!("start ffmpeg: {e}"))?;
        tracing::info!(channel = %channel.name, provider = channel.provider.as_str(), "ingest started");
        let mut inner = self.lock();
        inner.generation += 1;
        inner.current = Some(channel.clone());
        inner.run = Some(Run { child, folder: folder.clone(), carry: String::new() });
        inner.started_at = Some(Instant::now());
        inner.started_wall = Some(super::now_ms());
        inner.phase = Phase::Running;
        inner.last_error = None;
        inner.restart_at = None;
        inner.ready = false;
        inner.kbps = 0.0;
        inner.last_segment_at = None;
        inner.health.on_start();
        inner.resolution.clear();
        inner.last_resolution = None;
        inner.resolution_probe = None;
        if !relaunch {
            inner.restarts = 0;
        }
        drop(inner);
        self.clear_public_except(Some(&folder));
        Ok(())
    }

    pub(crate) async fn stop_run(&self) {
        let run = {
            let mut inner = self.lock();
            inner.generation += 1;
            inner.run.take()
        };
        let Some(run) = run else { return };
        stop_child(&run.child).await;
        let _ = std::fs::remove_dir_all(self.folder_path(&run.folder));
    }
}

pub(crate) async fn stop_child(child: &Child) {
    if child.try_wait().ok().flatten().is_some() {
        return;
    }
    child.terminate();
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    child.kill();
}

pub(crate) async fn wait_for_exit(child: &Child, limit: Duration) -> Option<i32> {
    let deadline = Instant::now() + limit;
    loop {
        if let Some(code) = child.try_wait().ok().flatten() {
            return Some(code);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(POLL).await;
    }
}

pub(crate) fn drained(child: &Child) -> (String, String) {
    (
        String::from_utf8_lossy(&child.read_stdout(1 << 20)).into_owned(),
        redact(&String::from_utf8_lossy(&child.read_stderr(1 << 20))),
    )
}

impl Live {
    pub fn shutdown(&self) {
        let run = {
            let mut inner = self.lock();
            inner.generation += 1;
            inner.phase = Phase::Paused;
            inner.run.take()
        };
        if let Some(run) = run {
            run.child.terminate();
            run.child.kill();
        }
        if let Some(probe) = self.lock().probe.take() {
            probe.child.kill();
        }
        self.clear_public_except(None);
        self.forget_buffer();
    }
}
