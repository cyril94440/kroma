use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::Command;
use serde::{Deserialize, Serialize};

use super::connections::{admit, Rule, Situation, Use};
use super::process::{drained, wait_for_exit};
use super::{now_ms, Live};
use crate::model::{Channel, ProviderId};
use crate::status::{ChannelRef, OffAirView, Phase};

const PROBE_LIMIT: Duration = Duration::from_secs(20);
const MAX_PLACEHOLDER_SECONDS: f64 = 14_400.0;
const ARRIVAL_WAIT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default)]
pub struct SwitchRequest {
    pub channel_id: String,
    pub skip_if_same: bool,
    pub fast: bool,
    pub only_if_paused: bool,
    pub requested_by: Option<String>,
    pub reason: String,
    pub only_if_current: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SwitchOutcome {
    pub status: String,
    pub channel: Option<ChannelRef>,
    pub error: Option<String>,
    pub duration: Option<f64>,
    pub restored: Option<String>,
}

impl SwitchOutcome {
    fn of(status: &str, channel: Option<&Channel>) -> SwitchOutcome {
        SwitchOutcome { status: status.into(), channel: channel.map(|c| ChannelRef { id: c.id(), name: c.name.clone() }), ..SwitchOutcome::default() }
    }

    fn error(message: impl Into<String>) -> SwitchOutcome {
        SwitchOutcome { status: "error".into(), error: Some(message.into()), ..SwitchOutcome::default() }
    }

    pub fn switched(&self) -> bool {
        self.status == "switched" || self.status == "already"
    }
}

impl Live {
    pub async fn probe_live(&self, channel: &Channel, url: &str, user_agent: &str) -> (bool, Option<f64>) {
        if !url.starts_with("http") {
            return (true, None);
        }
        let spawned = Command::new(self.programs.ffprobe.clone())
            .args(["-v", "error", "-user_agent", user_agent, "-show_entries", "format=duration:stream=codec_type,codec_name", "-of", "json", url])
            .spawn();
        let Ok(child) = spawned else { return (true, None) };
        let finished = wait_for_exit(&child, PROBE_LIMIT).await;
        if finished.is_none() {
            child.kill();
            return (true, None);
        }
        let (stdout, _) = drained(&child);
        let report = serde_json::from_str::<serde_json::Value>(&stdout).ok();
        let video = report.as_ref().and_then(|v| v.pointer("/streams")).and_then(|s| s.as_array()).and_then(|streams| streams.iter().find(|s| s["codec_type"] == "video")).and_then(|s| s["codec_name"].as_str().map(str::to_string));
        if let Some(name) = video {
            self.note_codec(channel, super::codec::VideoCodec::named(&name));
        }
        let duration = report.and_then(|v| v.pointer("/format/duration").and_then(|d| d.as_str().and_then(|s| s.trim().parse::<f64>().ok()).or_else(|| d.as_f64())));
        match duration {
            Some(d) => (d > MAX_PLACEHOLDER_SECONDS, Some(d)),
            None => (true, None),
        }
    }

    pub async fn switch(&self, request: SwitchRequest) -> SwitchOutcome {
        let _gate = self.gate.lock().await;
        let Some(target) = self.providers.find_by_ref(&request.channel_id) else {
            return SwitchOutcome::error("unknown channel");
        };
        let target = self.route_around_outage(target);
        let outcome = self.switch_locked(&target, &request).await;
        self.publish_status();
        outcome
    }

    async fn switch_locked(&self, target: &Channel, request: &SwitchRequest) -> SwitchOutcome {
        if request.only_if_paused {
            return self.defer_resume(target);
        }
        let current = self.current();
        if request.only_if_current.as_ref().is_some_and(|expected| current.as_ref().map(Channel::id).as_ref() != Some(expected)) {
            return SwitchOutcome::of("changed", current.as_ref());
        }
        if request.skip_if_same {
            let inner = self.lock();
            let same = inner.current.as_ref().is_some_and(|c| c.id() == target.id());
            if same && inner.phase != Phase::Paused && inner.run.is_some() {
                return SwitchOutcome::of("already", Some(target));
            }
        }
        self.stop_trial_marker();
        let settings = self.settings();
        let single = settings.single_connection();
        let fast = request.fast || (single.contains(&target.provider) && current.as_ref().is_some_and(|c| c.provider == target.provider));
        let same_url = current.as_ref().is_some_and(|c| c.id() == target.id());
        self.lock().arriving = true;
        let outcome = self.switch_arrived(target, current.as_ref(), fast, same_url, &single).await;
        self.lock().arriving = false;
        outcome
    }

    async fn switch_arrived(&self, target: &Channel, current: Option<&Channel>, fast: bool, same_url: bool, single: &[ProviderId]) -> SwitchOutcome {
        self.wait_for_tests(target.provider, single).await;
        let began = Instant::now();
        let source = match self.source_for(target) {
            Ok(source) => source,
            Err(error) => return SwitchOutcome::error(error.to_string()),
        };
        let mut probe_done = began;
        let mut placeholder = None;
        if !fast && !same_url {
            let (live, duration) = self.probe_live(target, &source.url, source.user_agent.as_deref().unwrap_or("")).await;
            probe_done = Instant::now();
            if !live {
                let view = OffAirView { name: target.name.clone(), duration, at: now_ms() };
                if current.is_some() {
                    self.lock().off_air = Some(view);
                    return SwitchOutcome { duration, restored: current.map(|c| c.name.clone()), ..SwitchOutcome::of("offair", Some(target)) };
                }
                placeholder = Some(view);
            }
        }
        {
            let mut inner = self.lock();
            inner.off_air = placeholder;
            inner.phase = Phase::Switching;
            inner.black = false;
            inner.frozen = false;
            inner.streaks.reset();
            if !same_url {
                inner.health.on_channel_change();
            }
            inner.restarts = 0;
        }
        self.publish_status();
        let stop_began = Instant::now();
        self.stop_run().await;
        let stopped = Instant::now();
        if let Err(error) = self.start_run(target, false) {
            return SwitchOutcome::error(self.fail_start(&error));
        }
        let launched = Instant::now();
        {
            let mut inner = self.lock();
            inner.zap = Some(super::media::ZapTiming { started: Some(began), probe_done: Some(probe_done.max(stop_began)), stopped: Some(stopped), launched: Some(launched) });
            if inner.off_air.is_some() {
                inner.phase = Phase::OffAir;
            }
        }
        self.remember_current(false);
        self.publish_channel();
        SwitchOutcome::of("switched", Some(target))
    }

    async fn wait_for_tests(&self, provider: ProviderId, single: &[ProviderId]) {
        if !single.contains(&provider) {
            return;
        }
        let deadline = Instant::now() + ARRIVAL_WAIT;
        while self.lock().tests_running > 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    fn defer_resume(&self, target: &Channel) -> SwitchOutcome {
        let mut inner = self.lock();
        if inner.phase != Phase::Paused {
            return SwitchOutcome::of("not_paused", None);
        }
        inner.current = Some(target.clone());
        inner.off_air = None;
        inner.health.on_channel_change();
        inner.streaks.reset();
        drop(inner);
        self.remember_current(true);
        SwitchOutcome::of("deferred", Some(target))
    }

    pub async fn pause(&self) -> &'static str {
        let Ok(_gate) = tokio::time::timeout(Duration::from_secs(1), self.gate.lock()).await else {
            return "busy";
        };
        if self.lock().phase == Phase::Paused {
            return "already_paused";
        }
        self.stop_trial_marker();
        self.stop_run().await;
        {
            let mut inner = self.lock();
            inner.phase = Phase::Paused;
            inner.health.on_pause();
            inner.ready = false;
            inner.kbps = 0.0;
            inner.restart_at = None;
        }
        self.remember_current(true);
        self.publish_status();
        "paused"
    }

    pub async fn resume(&self) -> SwitchOutcome {
        let _gate = self.gate.lock().await;
        let current = {
            let inner = self.lock();
            if inner.phase != Phase::Paused {
                return SwitchOutcome::of("already_active", inner.current.as_ref());
            }
            inner.current.clone()
        };
        let Some(channel) = current.or_else(|| self.default_channel()) else {
            return SwitchOutcome::of("no_channel", None);
        };
        self.stop_trial_marker();
        self.lock().arriving = true;
        let single = self.settings().single_connection();
        self.wait_for_tests(channel.provider, &single).await;
        let started = self.start_run(&channel, false);
        self.lock().arriving = false;
        let outcome = match started {
            Ok(()) => {
                self.remember_current(false);
                self.publish_channel();
                SwitchOutcome::of("resumed", Some(&channel))
            }
            Err(error) => {
                let message = crate::portal::redact(&format!("{error:#}"));
                let mut inner = self.lock();
                inner.phase = Phase::Error;
                inner.last_error = Some(message.clone());
                SwitchOutcome::error(message)
            }
        };
        self.publish_status();
        outcome
    }

    fn route_around_outage(&self, target: Channel) -> Channel {
        let outage = {
            let inner = self.lock();
            target.provider == ProviderId::Primary && inner.policy.zap_provider() == ProviderId::Backup
        };
        if !outage {
            return target;
        }
        let rules = self.rules();
        let to = self.lock().policy.zap_provider();
        self.providers.mapped(&target, to, &rules).unwrap_or(target)
    }

    pub fn default_channel(&self) -> Option<Channel> {
        let (settings, rules) = (self.settings(), self.rules());
        let wanted = settings.default_channel;
        if wanted.is_empty() {
            return None;
        }
        self.providers.catalogue(ProviderId::Primary).find(&wanted, &rules).cloned()
    }

    pub fn admission(&self, kind: Use, target: ProviderId) -> Result<(), super::Refusal> {
        let settings = self.settings();
        let single = settings.single_connection();
        let inner = self.lock();
        let live = inner.current.as_ref().map(|c| c.provider);
        let situation = Situation { live, tests_at_target: inner.tests_running, arriving_at_target: inner.arriving };
        admit(kind, target, &situation, &Rule { single: &single, scan_with_live: settings.backup_scan_with_live })
    }
}
