use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::Command;

use super::process::{stop_child, wait_for_exit};
use super::{now_ms, Live};
use crate::clipbuf::{self, ClipFile, BUFFER_DIR, CLIPS_DIR, CLIPS_MAX_BYTES, CLIPS_MAX_DAYS, INIT_FILE};
use crate::cmd::PLAYLIST_FILE;

pub const WORK_DIR: &str = "work";
const CUT_LIMIT: Duration = Duration::from_secs(90);
const PURGE_BUFFER_EVERY: Duration = Duration::from_secs(10);
const PURGE_CLIPS_EVERY: Duration = Duration::from_secs(3600);
const MIN_CLIP_BYTES: u64 = 50_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipCut {
    pub path: String,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipError {
    InvalidName,
    InvalidWindow,
    OutOfBuffer,
    CutFailed,
}

impl ClipError {
    pub fn as_str(self) -> &'static str {
        match self {
            ClipError::InvalidName => "invalidName",
            ClipError::InvalidWindow => "invalidWindow",
            ClipError::OutOfBuffer => "outOfBuffer",
            ClipError::CutFailed => "cutFailed",
        }
    }
}

fn link_or_copy(from: &Path, to: &Path) -> Option<u64> {
    if let Some(folder) = to.parent() {
        std::fs::create_dir_all(folder).ok()?;
    }
    if !to.exists() && std::fs::hard_link(from, to).is_err() {
        std::fs::copy(from, to).ok()?;
    }
    std::fs::metadata(to).ok().map(|meta| meta.len())
}

fn seconds_of(modified: std::time::SystemTime) -> i64 {
    modified.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

impl Live {
    pub(crate) fn buffer_path(&self) -> PathBuf {
        self.root.join(BUFFER_DIR)
    }

    pub(crate) fn clips_path(&self) -> PathBuf {
        self.public_dir().join(CLIPS_DIR)
    }

    pub fn clip_buffer_info(&self) -> Option<(i64, i64)> {
        self.lock().buffer.bounds()
    }

    #[cfg(test)]
    pub(crate) fn tick_blocking_buffer(&self) {
        self.buffer_step();
    }

    pub(crate) fn buffer_step(&self) {
        let current = self.lock().run.as_ref().map(|run| run.folder.clone());
        if let Some(folder) = current.as_deref() {
            let source = self.folder_path(folder);
            if let Ok(text) = std::fs::read_to_string(source.join(PLAYLIST_FILE)) {
                let target = self.buffer_path().join(folder);
                let _ = link_or_copy(&source.join(INIT_FILE), &target.join(INIT_FILE));
                self.lock().buffer.observe(folder, &target, &text, |name| link_or_copy(&source.join(name), &target.join(name)));
            }
        }
        let due = self.lock().last_buffer_purge.is_none_or(|at| at.elapsed() >= PURGE_BUFFER_EVERY);
        if due {
            self.purge_buffer(current.as_deref());
        }
        if self.lock().last_clip_purge.is_none_or(|at| at.elapsed() >= PURGE_CLIPS_EVERY) {
            self.purge_clips();
        }
    }

    fn purge_buffer(&self, current: Option<&str>) {
        let purged = {
            let mut inner = self.lock();
            inner.last_buffer_purge = Some(Instant::now());
            inner.buffer.purge(now_ms(), current)
        };
        for file in purged.files {
            let _ = std::fs::remove_file(file);
        }
        for folder in purged.folders {
            let _ = std::fs::remove_dir_all(folder);
        }
    }

    pub(crate) fn purge_clips(&self) {
        self.lock().last_clip_purge = Some(Instant::now());
        let root = self.clips_path();
        let Ok(entries) = std::fs::read_dir(&root) else { return };
        let mut clips = Vec::new();
        for folder in entries.flatten().filter(|e| e.path().is_dir()) {
            for file in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
                if let Ok(meta) = file.metadata() {
                    clips.push(ClipFile { path: file.path(), modified_s: meta.modified().map_or(0, seconds_of), bytes: meta.len() });
                }
            }
        }
        for path in clipbuf::clips_to_purge(clips, now_ms() / 1000, CLIPS_MAX_BYTES, CLIPS_MAX_DAYS) {
            let _ = std::fs::remove_file(&path);
            if let Some(folder) = path.parent() {
                let _ = std::fs::remove_dir(folder);
            }
        }
    }

    pub(crate) fn forget_buffer(&self) {
        let _ = std::fs::remove_dir_all(self.buffer_path());
        let _ = std::fs::remove_dir_all(self.root.join(WORK_DIR));
        self.lock().buffer = clipbuf::Buffer::default();
    }

    pub async fn clip_cut(&self, from_ms: i64, to_ms: i64, name: &str) -> Result<ClipCut, ClipError> {
        if !clipbuf::valid_name(name) {
            return Err(ClipError::InvalidName);
        }
        if !clipbuf::valid_window(from_ms, to_ms) {
            return Err(ClipError::InvalidWindow);
        }
        let selection = self.lock().buffer.choose(from_ms, to_ms, None).ok_or(ClipError::OutOfBuffer)?;
        let token = kroma_module_sdk::primitives::random_token();
        let work = self.root.join(WORK_DIR);
        std::fs::create_dir_all(&work).map_err(|_| ClipError::CutFailed)?;
        let playlist = work.join(format!("{token}.m3u8"));
        let partial = work.join(format!("{token}.mp4"));
        let outcome = self.run_cut(&selection.playlist(), &playlist, &partial).await;
        let _ = std::fs::remove_file(&playlist);
        if !outcome {
            let _ = std::fs::remove_file(&partial);
            return Err(ClipError::CutFailed);
        }
        let folder = self.clips_path().join(&token);
        let final_file = folder.join(format!("{name}.mp4"));
        let placed = std::fs::create_dir_all(&folder).and_then(|()| std::fs::rename(&partial, &final_file));
        if placed.is_err() {
            let _ = std::fs::remove_file(&partial);
            let _ = std::fs::remove_dir_all(&folder);
            return Err(ClipError::CutFailed);
        }
        self.purge_clips();
        Ok(ClipCut { path: format!("{CLIPS_DIR}/{token}/{name}.mp4"), duration_ms: selection.duration_ms() })
    }

    async fn run_cut(&self, playlist_text: &str, playlist: &Path, output: &Path) -> bool {
        if std::fs::write(playlist, playlist_text).is_err() {
            return false;
        }
        let args = clipbuf::cut_args(&playlist.to_string_lossy(), &output.to_string_lossy());
        let Ok(child) = Command::new(self.programs.ffmpeg.clone()).args(args).spawn() else { return false };
        match wait_for_exit(&child, CUT_LIMIT).await {
            Some(0) => std::fs::metadata(output).is_ok_and(|meta| meta.len() >= MIN_CLIP_BYTES),
            Some(_) => false,
            None => {
                stop_child(&child).await;
                false
            }
        }
    }
}
