use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use anyhow::Result;
use kroma_module_sdk::host::run_blocking;

use super::disk;
use super::job::FileJob;
use crate::api::Api;
use crate::files::Planned;

const RETRY_AFTER: Duration = Duration::from_secs(30);
const PAUSE_BETWEEN_STEPS: Duration = Duration::from_millis(1);

pub struct Pull {
    dest: PathBuf,
    inner: Mutex<Inner>,
    cancelled: AtomicBool,
}

struct Inner {
    total: u64,
    done: u64,
    session: u64,
    started: Instant,
    phase: Phase,
}

enum Phase {
    Running,
    Failed(String, Instant),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Running { done: u64, total: u64, bps: u64 },
    Failed(String),
}

fn registry() -> &'static Mutex<HashMap<PathBuf, Arc<Pull>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<Pull>>>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}

fn held() -> MutexGuard<'static, HashMap<PathBuf, Arc<Pull>>> {
    registry().lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn running() -> HashSet<PathBuf> {
    held()
        .keys()
        .flat_map(|dest| [disk::part_of(dest), disk::chunk_of(dest)])
        .collect()
}

pub fn load_under(root: &Path) -> super::reserve::Load {
    let held = held();
    let mine = held.iter().filter(|(dest, _)| dest.starts_with(root));
    let (pulls, unwritten) = mine.fold((0, 0u64), |(pulls, left), (_, pull)| {
        let inner = pull.state();
        let remaining = match inner.phase {
            Phase::Running => inner.total.saturating_sub(inner.done),
            Phase::Failed(..) => 0,
        };
        (pulls + usize::from(matches!(inner.phase, Phase::Running)), left + remaining)
    });
    super::reserve::Load { pulls, unwritten }
}

pub fn is_running(dest: &Path) -> bool {
    held().get(dest).is_some_and(|pull| pull.current().is_some())
}

pub fn forget(dest: &Path) {
    if let Some(gone) = held().remove(dest) {
        gone.cancelled.store(true, Ordering::Relaxed);
    }
}

pub fn forget_under(folder: &Path) {
    let inside: Vec<PathBuf> = held()
        .keys()
        .filter(|dest| dest.starts_with(folder))
        .cloned()
        .collect();
    for dest in inside {
        forget(&dest);
    }
}

/// Joins the fetch of `dest` already running, or starts one. A fetch that
/// failed answers its failure for a while before a new one is tried.
pub fn join_or_start(dest: &Path, file: &Planned, api: Api, after: Option<Tidy>) -> Progress {
    let existing = held().get(dest).cloned();
    if let Some(pull) = existing {
        if let Some(progress) = pull.current() {
            return progress;
        }
        forget(dest);
    }
    let started = disk::len(&disk::part_of(dest)).unwrap_or(0).min(file.size);
    let pull = Arc::new(Pull {
        dest: dest.to_path_buf(),
        inner: Mutex::new(Inner {
            total: file.size,
            done: started,
            session: 0,
            started: Instant::now(),
            phase: Phase::Running,
        }),
        cancelled: AtomicBool::new(false),
    });
    held().insert(dest.to_path_buf(), pull.clone());
    let job = FileJob::new(api, dest.to_path_buf(), file);
    tokio::spawn(run(pull.clone(), job, after));
    pull.current().unwrap_or(Progress::Running {
        done: started,
        total: file.size,
        bps: 0,
    })
}

impl Pull {
    fn state(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn current(&self) -> Option<Progress> {
        let inner = self.state();
        match &inner.phase {
            Phase::Running => Some(Progress::Running {
                done: inner.done,
                total: inner.total,
                bps: inner.session / inner.started.elapsed().as_secs().max(1),
            }),
            Phase::Failed(reason, at) if at.elapsed() < RETRY_AFTER => {
                Some(Progress::Failed(reason.clone()))
            }
            Phase::Failed(..) => None,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn moved(&self, bytes: u64) {
        let mut inner = self.state();
        inner.done += bytes;
        inner.session += bytes;
    }

    fn fail(&self, reason: String) {
        if !self.cancelled() {
            self.state().phase = Phase::Failed(reason, Instant::now());
        }
    }
}

pub struct Tidy {
    pub root: PathBuf,
    pub limits: super::purge::Limits,
}

async fn run(pull: Arc<Pull>, job: FileJob, after: Option<Tidy>) {
    match advance_to_the_end(&pull, job).await {
        Ok(()) => {
            if !pull.cancelled() {
                held().remove(&pull.dest);
                if let Some(tidy) = after {
                    super::purge::purge(&tidy.root, std::time::SystemTime::now(), tidy.limits, &running());
                }
            }
        }
        Err(error) => pull.fail(format!("{error:#}")),
    }
}

async fn advance_to_the_end(pull: &Arc<Pull>, mut job: FileJob) -> Result<()> {
    while !job.is_complete() {
        if pull.cancelled() {
            return Ok(());
        }
        let (back, step) = run_blocking(move || {
            let step = job.advance();
            (job, step)
        })
        .await
        .map_err(|_| anyhow::anyhow!("the fetch worker stopped"))?;
        job = back;
        let step = step?;
        pull.moved(step.moved);
        if step.finished {
            break;
        }
        tokio::time::sleep(PAUSE_BETWEEN_STEPS).await;
    }
    Ok(())
}
