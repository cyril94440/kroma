use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::{Child, Command};

use super::process::drained;
use super::Live;
use crate::cmd::PLAYLIST_FILE;
use crate::probe::{analyse, frame_count, needs_full_decode, probe_args, Verdict, FULL_SECONDS, MIN_KEY_FRAMES, SAMPLE_SECONDS};

pub const INTERVAL: Duration = Duration::from_secs(10);
pub const AFTER_START: Duration = Duration::from_secs(15);

pub struct ProbeRun {
    pub child: Child,
    pub full: bool,
    pub generation: u64,
    pub started: Instant,
    pub key_report: Option<String>,
}

impl Live {
    pub(crate) fn probe_step(&self) {
        let finished = {
            let inner = self.lock();
            inner.probe.as_ref().and_then(|p| p.child.try_wait().ok().flatten().map(|_| ())).is_some()
                || inner.probe.as_ref().is_some_and(|p| p.started.elapsed() > Duration::from_secs(40))
        };
        if finished {
            self.finish_probe();
            return;
        }
        let start = {
            let inner = self.lock();
            let due = inner.last_probe.is_none_or(|at| at.elapsed() >= INTERVAL);
            let settled = inner.started_at.is_some_and(|at| at.elapsed() >= AFTER_START);
            inner.probe.is_none() && due && settled && inner.ready && inner.phase == crate::status::Phase::Running && inner.run.is_some()
        };
        if start {
            self.start_probe(false, None);
        }
    }

    fn local_source(&self) -> Option<String> {
        let inner = self.lock();
        let run = inner.run.as_ref()?;
        Some(self.folder_path(&run.folder).join(PLAYLIST_FILE).to_string_lossy().into_owned())
    }

    fn start_probe(&self, full: bool, key_report: Option<String>) {
        let Some(source) = self.local_source() else { return };
        let mut args = probe_args(&source, full);
        args.splice(0..0, ["-allowed_extensions".to_string(), "ALL".to_string()]);
        let Ok(child) = Command::new(self.programs.ffmpeg.clone()).args(args).spawn() else { return };
        let mut inner = self.lock();
        let generation = inner.generation;
        inner.last_probe = Some(Instant::now());
        inner.probe = Some(ProbeRun { child, full, generation, started: Instant::now(), key_report });
    }

    fn finish_probe(&self) {
        let Some(run) = self.lock().probe.take() else { return };
        if run.child.try_wait().ok().flatten().is_none() {
            run.child.kill();
        }
        let (_, report) = drained(&run.child);
        let generation_now = self.lock().generation;
        if generation_now != run.generation {
            return;
        }
        let (window, min) = if run.full { (FULL_SECONDS, 2) } else { (SAMPLE_SECONDS, MIN_KEY_FRAMES) };
        let verdict = analyse(&report, window, min, !run.full);
        if !run.full && needs_full_decode(&report, verdict) && frame_count(&report) >= 1 {
            self.start_probe(true, Some(report));
            return;
        }
        let verdict = if run.full { verdict.or_else(|| run.key_report.as_deref().and_then(|r| analyse(r, SAMPLE_SECONDS, MIN_KEY_FRAMES, true))) } else { verdict };
        self.apply_verdict(verdict);
    }

    pub(crate) fn apply_verdict(&self, verdict: Option<Verdict>) {
        let Some(verdict) = verdict else { return };
        let mut inner = self.lock();
        let (black, frozen) = inner.streaks.record(verdict);
        inner.black = black;
        inner.frozen = frozen;
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use std::time::Duration;

    use crate::it_live::{load, rig, until, Rig};
    use crate::it_stream::playing;

    async fn probed(rig: &Rig) {
        rig.live.start_probe(false, None);
        assert!(until(Duration::from_secs(30), || rig.live.lock().probe.as_ref().is_none_or(|p| p.child.try_wait().ok().flatten().is_some())).await);
        rig.live.finish_probe();
    }

    async fn two_probes_of(id: &str) -> (bool, bool) {
        let rig = rig();
        load(&rig);
        playing(&rig, id).await;
        tokio::time::sleep(Duration::from_secs(8)).await;
        probed(&rig).await;
        probed(&rig).await;
        let inner = rig.live.lock();
        let verdict = (inner.black, inner.frozen);
        drop(inner);
        rig.live.shutdown();
        verdict
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_black_picture_is_confirmed_by_two_real_probes_of_the_local_playlist() {
        assert_eq!(two_probes_of("p:6").await, (true, false));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_motionless_picture_is_confirmed_frozen_and_a_moving_one_is_neither() {
        assert_eq!(two_probes_of("p:3").await, (false, true));
        assert_eq!(two_probes_of("p:1").await, (false, false));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_single_reading_confirms_nothing() {
        let rig = rig();
        load(&rig);
        playing(&rig, "p:6").await;
        tokio::time::sleep(Duration::from_secs(8)).await;

        probed(&rig).await;

        assert!(!rig.live.lock().black);
        rig.live.shutdown();
    }
}
