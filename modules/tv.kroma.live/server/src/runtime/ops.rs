use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::Command;
use serde::Serialize;
use serde_json::json;

use super::media;
use super::process::{stop_child, wait_for_exit};
use super::quick::{local_args, Probed};
use super::snapshot::TOPIC_FAILOVER;
use super::switch::SwitchRequest;
use super::{now_ms, now_seconds, Live};
use crate::db::{self, ProviderEvent};
use crate::health::{Cause, State};
use crate::model::{Channel, ProviderId};
use crate::status::Phase;

pub const RESTART_COOLDOWN: Duration = Duration::from_secs(300);
const MEASURE_SECONDS: &str = "9";
const MEASURE_LIMIT: Duration = Duration::from_secs(20);
const MEASURE_SKIP_SECONDS: f64 = 3.0;
const MEASURE_MIN_SPAN: f64 = 2.0;
const MEASURE_MIN_BYTES: u64 = 65_536;

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Restart {
    pub restarted: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSwitch {
    pub switched: bool,
    pub provider: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    pub channel_id: String,
    pub name: String,
    pub provider: Option<ProviderId>,
    pub alive: bool,
    pub resolution: Option<String>,
    pub codec: Option<String>,
    pub audio_codec: Option<String>,
    pub fps: Option<f64>,
    pub bitrate_kbps: Option<f64>,
    pub off_air: bool,
    pub local: bool,
    pub error: Option<String>,
}

pub fn measure_args(url: &str, user_agent: &str, output: &str) -> Vec<String> {
    ["-v", "error", "-nostdin", "-user_agent", user_agent, "-t", MEASURE_SECONDS, "-i", url, "-map", "0:v:0", "-c", "copy", "-progress", "pipe:1", "-stats_period", "1", "-f", "mpegts", "-y", output].map(String::from).to_vec()
}

pub fn rate_from_samples(samples: &[(f64, u64)]) -> Option<f64> {
    let first = samples.iter().find(|(seconds, _)| *seconds >= MEASURE_SKIP_SECONDS)?;
    let last = samples.last()?;
    let span = last.0 - first.0;
    let bytes = last.1.checked_sub(first.1)?;
    (span > MEASURE_MIN_SPAN && bytes > MEASURE_MIN_BYTES).then(|| bytes as f64 * 8.0 / span / 1000.0)
}

fn size_of_line(line: &str) -> Option<u64> {
    line.strip_prefix("total_size=")?.trim().parse().ok()
}

fn report_of(channel: &Channel, probed: Probed) -> ProbeReport {
    ProbeReport {
        channel_id: channel.id(),
        name: channel.name.clone(),
        provider: Some(channel.provider),
        alive: probed.alive,
        resolution: probed.resolution.clone(),
        codec: probed.video_codec.clone(),
        audio_codec: probed.audio_codec.clone(),
        fps: probed.fps,
        bitrate_kbps: None,
        off_air: probed.alive && probed.off_air(),
        local: false,
        error: if probed.alive { None } else { probed.reason },
    }
}

impl Live {
    pub async fn restart(&self, requested_by: &str, reason: &str) -> Restart {
        let refusal = |code: &str| Restart { restarted: false, error: Some(code.into()) };
        let min_kbps = self.settings().min_kbps;
        let (channel, evidence, alone) = {
            let inner = self.lock();
            let Some(channel) = inner.current.clone() else { return refusal("noChannel") };
            if inner.phase == Phase::Paused {
                return refusal("standby");
            }
            let running = inner.run.is_some();
            let stuttering = matches!(inner.health.state, State::Degraded | State::Severe) && inner.health.cause() != Cause::Local;
            let evidence = !running || inner.kbps <= min_kbps || inner.black || inner.frozen || inner.health.frozen || stuttering || inner.restarts >= 1;
            let alone = inner.presence.viewers().iter().all(|viewer| viewer.user_id == requested_by);
            (channel, evidence, alone)
        };
        if !(evidence || alone) {
            return refusal("streamHealthy");
        }
        if self.lock().last_restart.is_some_and(|at| at.elapsed() < RESTART_COOLDOWN) {
            return refusal("recentRestart");
        }
        self.lock().last_restart = Some(Instant::now());
        let outcome = self.switch(SwitchRequest { channel_id: channel.id(), fast: true, requested_by: Some(requested_by.into()), reason: reason.into(), only_if_current: Some(channel.id()), ..SwitchRequest::default() }).await;
        let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "restart".into(), channel: Some(channel.name.clone()), detail: outcome.status.clone(), ..ProviderEvent::default() });
        match outcome.status.as_str() {
            "switched" => Restart { restarted: true, error: None },
            "changed" => refusal("channelChanged"),
            _ => Restart { restarted: false, error: Some(outcome.error.unwrap_or(outcome.status)) },
        }
    }

    pub async fn switch_provider(&self, target: ProviderId, requested_by: &str, reason: &str) -> ProviderSwitch {
        let answer = |switched: bool, error: Option<&str>| ProviderSwitch { switched, provider: target.as_str().into(), error: error.map(str::to_string) };
        if !self.settings().has_backup() {
            return answer(false, Some("noBackup"));
        }
        let Some(current) = self.current() else { return answer(false, Some("noChannel")) };
        if current.provider == target {
            return answer(false, Some("alreadyOn"));
        }
        if target == ProviderId::Primary && self.lock().policy.primary_is_down() {
            return answer(false, Some("primaryDown"));
        }
        self.stop_trial_marker();
        let wanted = match self.working_equivalent(&current, target).await {
            Ok(channel) => channel,
            Err(reasons) => {
                let none = self.providers.equivalents(&current, target, 1, &self.rules()).is_empty();
                let busy = !reasons.is_empty() && reasons.iter().all(|r| r == "busy" || r == "liveAtThisProvider");
                return answer(false, Some(if none { "notFound" } else if busy { "busy" } else { "notWorking" }));
            }
        };
        let event = self.providers.catalogue(current.provider).is_event_channel(&current.name);
        let (alone, trouble) = {
            let inner = self.lock();
            (inner.presence.viewers().iter().all(|viewer| viewer.user_id == requested_by), inner.policy.trouble(now_seconds()))
        };
        if !event && (alone || trouble) {
            let outcome = self.switch(SwitchRequest { channel_id: wanted.id(), skip_if_same: true, requested_by: Some(requested_by.into()), reason: reason.into(), only_if_current: Some(current.id()), ..SwitchRequest::default() }).await;
            let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "providerSwitch".into(), from: Some(current.provider.as_str().into()), to: Some(target.as_str().into()), channel: Some(current.name.clone()), detail: reason.into(), ..ProviderEvent::default() });
            self.emit(TOPIC_FAILOVER, json!({ "kind": "providerSwitch", "from": current.provider, "to": target, "channel": current.name, "requestedBy": requested_by, "status": outcome.status }));
            return if outcome.switched() { answer(true, None) } else { answer(false, Some(&outcome.error.unwrap_or(outcome.status))) };
        }
        match self.propose_channel("switchProvider", &wanted, if event { "unverifiedMultiplex" } else { "requested" }).await {
            super::actions::Proposed::Applied => answer(true, None),
            _ => answer(false, Some("voteRequired")),
        }
    }

    pub async fn probe_channel(&self, channel_id: &str) -> ProbeReport {
        let Some(channel) = self.providers.find_by_ref(channel_id) else { return ProbeReport { channel_id: channel_id.into(), error: Some("unknownChannel".into()), ..ProbeReport::default() } };
        if self.current().is_some_and(|c| c.id() == channel.id()) {
            return self.probe_local(&channel).await;
        }
        let probed = self.quick_probe(&channel).await;
        let alive = probed.alive;
        let mut report = report_of(&channel, probed);
        if alive {
            report.bitrate_kbps = self.measure_kbps(&channel).await;
        }
        report
    }

    async fn probe_local(&self, channel: &Channel) -> ProbeReport {
        let (playlist, kbps, phase) = {
            let inner = self.lock();
            (inner.run.as_ref().filter(|_| inner.ready).map(|run| self.folder_path(&run.folder).join(crate::cmd::PLAYLIST_FILE)), inner.kbps, inner.phase)
        };
        let Some(playlist) = playlist else {
            let error = if phase == Phase::Paused { "standby" } else { "notReady" };
            return ProbeReport { channel_id: channel.id(), name: channel.name.clone(), provider: Some(channel.provider), local: true, error: Some(error.into()), ..ProbeReport::default() };
        };
        let probed = self.run_probe(local_args(&playlist.to_string_lossy())).await;
        let mut report = report_of(channel, Probed { duration: None, ..probed });
        report.local = true;
        report.bitrate_kbps = (kbps > 0.0).then_some((kbps * 10.0).round() / 10.0);
        report
    }

    async fn measure_kbps(&self, channel: &Channel) -> Option<f64> {
        let source = self.source_for(channel).ok()?;
        self.lock().tests_running += 1;
        let rate = self.run_measure(&source.url, source.user_agent.as_deref().unwrap_or("")).await;
        {
            let mut inner = self.lock();
            inner.tests_running = inner.tests_running.saturating_sub(1);
        }
        rate.map(|kbps| (kbps * 10.0).round() / 10.0)
    }

    async fn run_measure(&self, url: &str, user_agent: &str) -> Option<f64> {
        let work = self.root.join(super::clips::WORK_DIR);
        std::fs::create_dir_all(&work).ok()?;
        let output = work.join(format!("measure-{}.ts", kroma_module_sdk::primitives::random_token()));
        let child = Command::new(self.programs.ffmpeg.clone()).args(measure_args(url, user_agent, &output.to_string_lossy())).spawn().ok()?;
        let deadline = Instant::now() + MEASURE_LIMIT;
        let mut carry = String::new();
        let mut size = 0;
        let mut samples = Vec::new();
        loop {
            let finished = child.try_wait().ok().flatten().is_some();
            for line in media::lines(&mut carry, &media::drain_with(|cap| child.read_stdout(cap))) {
                if let Some(bytes) = size_of_line(&line) {
                    size = bytes;
                } else if let Some(seconds) = media::out_time(&line) {
                    samples.push((seconds, size));
                }
            }
            if finished || Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        if wait_for_exit(&child, Duration::ZERO).await.is_none() {
            stop_child(&child).await;
        }
        let _ = std::fs::remove_file(&output);
        rate_from_samples(&samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_counts_bytes_after_the_first_three_seconds_to_skip_the_buffer_burst() {
        let samples = [(1.0, 900_000), (2.0, 1_000_000), (3.0, 1_100_000), (6.0, 1_100_000 + 750_000), (9.0, 1_100_000 + 1_500_000)];

        let rate = rate_from_samples(&samples).unwrap();

        assert!((rate - 2000.0).abs() < 0.01, "{rate}");
    }

    #[test]
    fn too_short_or_too_light_a_measure_gives_nothing() {
        assert_eq!(rate_from_samples(&[(3.0, 0), (4.0, 1_000_000)]), None);
        assert_eq!(rate_from_samples(&[(3.0, 0), (9.0, 1000)]), None);
        assert_eq!(rate_from_samples(&[(1.0, 0), (2.0, 10_000_000)]), None);
        assert_eq!(rate_from_samples(&[]), None);
    }

    #[test]
    fn the_measure_copies_the_video_for_nine_seconds_into_a_muxer_that_counts_bytes() {
        let args = measure_args("http://x/1.ts", "UA", "/w/m.ts");

        assert_eq!(args[args.iter().position(|a| a == "-t").unwrap() + 1], "9");
        assert_eq!(&args[args.len() - 4..], ["-f", "mpegts", "-y", "/w/m.ts"]);
    }
}
