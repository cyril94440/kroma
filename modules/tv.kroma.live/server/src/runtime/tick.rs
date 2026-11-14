use std::sync::Arc;
use std::time::{Duration, Instant};

use super::media::{self, STALE_AFTER};
use super::process::SavedCurrent;
use super::{now_ms, Live, KV_CURRENT};
use crate::cmd::restart_delay_seconds;
use crate::db::{self, HistoryPoint, ZapRecord};
use crate::portal::redact;
use crate::status::Phase;

const TICK: Duration = Duration::from_millis(1000);
const MEDIA_EVERY: Duration = Duration::from_secs(3);
const HISTORY_EVERY: Duration = Duration::from_secs(10);
const RESTARTS_DECAY: Duration = Duration::from_secs(600);
const ZAP_EXPIRES: Duration = Duration::from_secs(90);
const HEALTH_EVERY: Duration = Duration::from_secs(5);

impl Live {
    pub fn start(self: &Arc<Self>) {
        let live = self.clone();
        tokio::spawn(async move {
            live.boot().await;
            loop {
                tokio::time::sleep(TICK).await;
                live.tick().await;
            }
        });
    }

    pub async fn boot(&self) {
        let _ = std::fs::create_dir_all(self.public_dir());
        self.clear_public_except(None);
        self.lock().presence.start_idle(now_ms());
        let settings = self.settings();
        if self.providers.catalogue(crate::model::ProviderId::Primary).is_empty() && !settings.primary_url.is_empty() {
            let _ = self.refresh_catalogues();
        }
        let saved: SavedCurrent = db::kv_get_json(&self.pool, KV_CURRENT).unwrap_or_default();
        let channel = self.providers.find_by_ref(&saved.reference);
        let Some(channel) = channel else { return };
        {
            let mut inner = self.lock();
            inner.current = Some(channel.clone());
            inner.phase = Phase::Paused;
        }
        if !saved.paused {
            if let Err(error) = self.start_run(&channel, false) {
                let mut inner = self.lock();
                inner.phase = Phase::Error;
                inner.last_error = Some(redact(&format!("{error:#}")));
            }
        }
        self.publish_status();
    }

    pub fn refresh_catalogues(&self) -> Vec<(crate::model::ProviderId, Result<usize, String>)> {
        let (settings, rules, now) = (self.settings(), self.rules(), now_ms());
        [crate::model::ProviderId::Primary, crate::model::ProviderId::Backup]
            .into_iter()
            .filter(|p| !settings.url_of(*p).is_empty())
            .map(|p| (p, self.providers.refresh(p, &settings, &rules, now).map_err(|e| e.to_string())))
            .collect()
    }

    pub(crate) async fn tick(self: &Arc<Self>) {
        self.read_run();
        self.restart_if_due();
        self.check_media();
        self.decay_restarts();
        self.presence_step().await;
        self.record_history();
        self.probe_step();
        self.failover_step().await;
        self.publish_status();
        let due = self.lock().last_health_at.is_none_or(|at| at.elapsed() >= HEALTH_EVERY);
        if due {
            self.lock().last_health_at = Some(Instant::now());
            self.publish_health(false);
        }
    }

    fn read_run(&self) {
        let mut inner = self.lock();
        let Some(run) = inner.run.as_mut() else { return };
        let stdout = run.child.read_stdout(1 << 16);
        let stderr = run.child.read_stderr(1 << 16);
        let exited = run.child.try_wait().ok().flatten();
        let mut carry = std::mem::take(&mut run.carry);
        let blocks: Vec<f64> = media::lines(&mut carry, &stdout).iter().filter_map(|l| media::out_time(l)).collect();
        if let Some(run) = inner.run.as_mut() {
            run.carry = carry;
        }
        for line in String::from_utf8_lossy(&stderr).lines() {
            let clean = redact(line.trim());
            if clean.is_empty() {
                continue;
            }
            inner.health.on_stderr(&clean);
            if inner.stderr_tail.len() >= 5 {
                inner.stderr_tail.pop_front();
            }
            inner.stderr_tail.push_back(clean);
        }
        for block in blocks {
            inner.health.on_block(block);
        }
        if let Some(code) = exited {
            let tail: Vec<String> = inner.stderr_tail.iter().rev().take(3).rev().cloned().collect();
            inner.last_error = Some(if tail.is_empty() { format!("exit code {code}") } else { tail.join(" | ") });
            inner.restarts += 1;
            inner.restarts_total += 1;
            let delay = restart_delay_seconds(inner.restarts);
            inner.restart_at = Some(Instant::now() + Duration::from_secs(delay));
            if let Some(dead) = inner.run.take() {
                let folder = self.folder_path(&dead.folder);
                drop(inner);
                let _ = std::fs::remove_dir_all(folder);
                inner = self.lock();
            }
            inner.ready = false;
            inner.kbps = 0.0;
            inner.generation += 1;
            if inner.restarts >= 3 {
                inner.phase = Phase::Error;
            }
            tracing::warn!(code, delay, "ffmpeg ended on its own; restarting");
        }
    }

    fn restart_if_due(&self) {
        let (due, channel) = {
            let inner = self.lock();
            let due = inner.restart_at.is_some_and(|at| Instant::now() >= at) && inner.run.is_none() && inner.phase != Phase::Paused;
            (due, inner.current.clone())
        };
        let Some(channel) = channel.filter(|_| due) else { return };
        let Ok(_gate) = self.gate.try_lock() else { return };
        if let Err(error) = self.start_run(&channel, true) {
            let mut inner = self.lock();
            inner.last_error = Some(redact(&format!("{error:#}")));
            inner.phase = Phase::Error;
            inner.restart_at = Some(Instant::now() + Duration::from_secs(restart_delay_seconds(inner.restarts.max(1))));
        }
    }

    fn check_media(&self) {
        let mut inner = self.lock();
        if inner.last_media_check.is_some_and(|at| at.elapsed() < MEDIA_EVERY) {
            return;
        }
        inner.last_media_check = Some(Instant::now());
        let Some(folder) = inner.run.as_ref().map(|r| self.folder_path(&r.folder)) else { return };
        let window = media::read_window(&folder);
        let stale = media::newest_segment_age(&folder).is_none_or(|age| age > STALE_AFTER);
        let ready = window.is_some_and(|w| w.segments > 0);
        inner.kbps = if ready && !stale { window.map_or(0.0, |w| w.kbps) } else { 0.0 };
        if ready && !inner.ready {
            inner.ready = true;
            if let Some(zap) = inner.zap.take() {
                if let (Some(started), Some(launched)) = (zap.started, zap.launched) {
                    let (probe, stopped) = (zap.probe_done.unwrap_or(started), zap.stopped.unwrap_or(started));
                    let ms = |a: Instant, b: Instant| b.saturating_duration_since(a).as_millis() as i64;
                    let record = ZapRecord { at: now_ms(), total_ms: started.elapsed().as_millis() as i64, probe_ms: ms(started, probe), stop_ms: ms(probe, stopped), launch_ms: ms(stopped, launched), publication_ms: launched.elapsed().as_millis() as i64 };
                    if started.elapsed() < ZAP_EXPIRES {
                        let _ = db::add_zap(&self.pool, &record);
                    }
                }
            }
        }
    }

    fn decay_restarts(&self) {
        let mut inner = self.lock();
        if inner.restarts > 0 && inner.run.is_some() && inner.started_at.is_some_and(|at| at.elapsed() > RESTARTS_DECAY) {
            inner.restarts = 0;
            if inner.phase == Phase::Error {
                inner.phase = Phase::Running;
            }
        }
    }

    async fn presence_step(&self) {
        let now = now_ms();
        let (expired, due, resume) = {
            let mut inner = self.lock();
            let expired = inner.presence.expire(now);
            let due = inner.presence.standby_due(now, self.settings().standby_seconds) && inner.phase != Phase::Paused && inner.current.is_some();
            let resume = inner.phase == Phase::Paused && inner.presence.count() > 0 && inner.current.is_some();
            (expired, due, resume)
        };
        if expired {
            self.publish_presence();
        }
        if due {
            self.pause_if_idle().await;
        }
        if resume {
            self.resume().await;
        }
    }

    pub(crate) async fn pause_if_idle(&self) -> &'static str {
        let Ok(gate) = tokio::time::timeout(Duration::from_secs(1), self.gate.lock()).await else { return "busy" };
        let still_idle = self.lock().presence.standby_due(now_ms(), self.settings().standby_seconds);
        drop(gate);
        if still_idle { self.pause_after_check().await } else { "viewers" }
    }

    async fn pause_after_check(&self) -> &'static str {
        if self.lock().presence.count() > 0 {
            return "viewers";
        }
        self.pause().await
    }

    fn record_history(&self) {
        let mut inner = self.lock();
        if inner.last_history.is_some_and(|at| at.elapsed() < HISTORY_EVERY) {
            return;
        }
        inner.last_history = Some(Instant::now());
        let point = HistoryPoint {
            at: now_ms() / 1000,
            kbps: inner.kbps as i64,
            channel: inner.current.as_ref().map(|c| c.name.clone()),
            provider: inner.current.as_ref().map(|c| c.provider.as_str().to_string()),
            health: Some(inner.health.state.as_str().to_string()),
            loss: inner.health.recent_loss(),
            frozen: inner.frozen,
            black: inner.black,
            restarts: inner.restarts_total as i64,
            paused: inner.phase == Phase::Paused,
        };
        drop(inner);
        let _ = db::add_history_point(&self.pool, &point);
    }
}
