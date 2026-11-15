use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::Command;

use super::clips::WORK_DIR;
use super::process::{stop_child, wait_for_exit};
use super::{now_ms, Live};
use crate::cmd::PLAYLIST_FILE;
use crate::status::Phase;

pub const SETTLE: Duration = Duration::from_secs(12);
#[cfg(test)]
pub const SETTLE_FOR_TESTS: Duration = SETTLE;
pub const REUSE: Duration = Duration::from_secs(15);
pub const MAX_HEIGHT: u32 = 720;
pub const TARGET_BYTES: usize = 300_000;
pub const HARD_BYTES: usize = 1024 * 1024;
const GRAB_LIMIT: Duration = Duration::from_secs(20);
const QUALITY: u32 = 5;
const LOW_QUALITY: u32 = 14;

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub jpeg: Vec<u8>,
    pub at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    Standby,
    Stopped,
    Settling,
    Changed,
    Failed,
    TooLarge,
}

impl FrameError {
    pub fn as_str(self) -> &'static str {
        match self {
            FrameError::Standby => "standby",
            FrameError::Stopped => "stopped",
            FrameError::Settling => "settling",
            FrameError::Changed => "changed",
            FrameError::Failed => "captureFailed",
            FrameError::TooLarge => "tooLarge",
        }
    }
}

pub(crate) struct CachedFrame {
    frame: Frame,
    channel: String,
    generation: u64,
    taken: Instant,
}

pub fn frame_args(playlist: &str, output: &str, quality: u32) -> Vec<String> {
    ["-v", "error", "-nostdin", "-y", "-allowed_extensions", "ALL", "-live_start_index", "-1", "-i", playlist, "-frames:v", "1", "-an", "-vf"].map(String::from).into_iter().chain([format!("scale=-2:'min({MAX_HEIGHT},ih)'"), "-q:v".into(), quality.to_string(), output.to_string()]).collect()
}

struct Spot {
    channel: String,
    generation: u64,
    playlist: String,
}

impl Live {
    fn frame_spot(&self) -> Result<Spot, FrameError> {
        let inner = self.lock();
        if inner.phase == Phase::Paused {
            return Err(FrameError::Standby);
        }
        let (Some(run), Some(channel)) = (inner.run.as_ref(), inner.current.as_ref()) else { return Err(FrameError::Stopped) };
        if !inner.ready || inner.started_at.is_none_or(|at| at.elapsed() < SETTLE) {
            return Err(FrameError::Settling);
        }
        Ok(Spot { channel: channel.id(), generation: inner.generation, playlist: self.folder_path(&run.folder).join(PLAYLIST_FILE).to_string_lossy().into_owned() })
    }

    pub async fn grab_frame(&self, fresh: bool) -> Result<Frame, FrameError> {
        let spot = self.frame_spot()?;
        let mut cache = self.frame.lock().await;
        if let Some(held) = cache.as_ref().filter(|c| !fresh && c.channel == spot.channel && c.generation == spot.generation && c.taken.elapsed() < REUSE) {
            return Ok(held.frame.clone());
        }
        let jpeg = self.capture_jpeg(&spot.playlist).await?;
        let after = self.frame_spot().map_err(|_| FrameError::Changed)?;
        if after.channel != spot.channel || after.generation != spot.generation {
            return Err(FrameError::Changed);
        }
        let frame = Frame { jpeg, at: now_ms() };
        *cache = Some(CachedFrame { frame: frame.clone(), channel: spot.channel, generation: spot.generation, taken: Instant::now() });
        Ok(frame)
    }

    async fn capture_jpeg(&self, playlist: &str) -> Result<Vec<u8>, FrameError> {
        let work = self.root.join(WORK_DIR);
        std::fs::create_dir_all(&work).map_err(|_| FrameError::Failed)?;
        let output = work.join(format!("frame-{}.jpg", kroma_module_sdk::primitives::random_token()));
        let mut result = Err(FrameError::Failed);
        for quality in [QUALITY, LOW_QUALITY] {
            result = self.run_grab(playlist, &output, quality).await;
            if result.as_ref().is_ok_and(|jpeg| jpeg.len() <= TARGET_BYTES) {
                break;
            }
        }
        let _ = std::fs::remove_file(&output);
        result.and_then(|jpeg| if jpeg.len() > HARD_BYTES { Err(FrameError::TooLarge) } else { Ok(jpeg) })
    }

    async fn run_grab(&self, playlist: &str, output: &std::path::Path, quality: u32) -> Result<Vec<u8>, FrameError> {
        let _ = std::fs::remove_file(output);
        let child = Command::new(self.programs.ffmpeg.clone()).args(frame_args(playlist, &output.to_string_lossy(), quality)).spawn().map_err(|_| FrameError::Failed)?;
        if wait_for_exit(&child, GRAB_LIMIT).await != Some(0) {
            stop_child(&child).await;
            return Err(FrameError::Failed);
        }
        std::fs::read(output).ok().filter(|jpeg| !jpeg.is_empty()).ok_or(FrameError::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grab_reads_the_local_playlist_at_its_live_edge_and_caps_the_height() {
        let args = frame_args("/p/index.m3u8", "/w/f.jpg", 5);

        assert_eq!(args[args.iter().position(|a| a == "-live_start_index").unwrap() + 1], "-1");
        assert_eq!(args[args.iter().position(|a| a == "-frames:v").unwrap() + 1], "1");
        assert_eq!(args[args.iter().position(|a| a == "-vf").unwrap() + 1], "scale=-2:'min(720,ih)'");
        assert_eq!(args.last().map(String::as_str), Some("/w/f.jpg"));
        assert!(args.contains(&"-an".to_string()));
    }
}
