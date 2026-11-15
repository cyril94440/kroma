use std::time::{Duration, Instant};

use kroma_module_sdk::host::process::{Child, Command};
use serde::Deserialize;

use super::codec::VideoCodec;
use super::process::drained;
use super::Live;
use crate::cmd::PLAYLIST_FILE;
use crate::status::Phase;

pub const RETRY_EVERY: Duration = Duration::from_secs(30);
const GIVE_UP_AFTER: Duration = Duration::from_secs(20);
const HVC1: &str = "hvc1";

pub struct ResolutionRun {
    pub child: Child,
    pub generation: u64,
    pub started: Instant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub resolution: String,
    pub codec: VideoCodec,
    pub tag: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Probe {
    streams: Vec<ProbeStream>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ProbeStream {
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: String,
    codec_name: String,
    codec_tag_string: String,
}

pub fn probe_args(playlist: &str) -> Vec<String> {
    ["-v", "error", "-allowed_extensions", "ALL", "-select_streams", "v:0", "-show_entries", "stream=width,height,avg_frame_rate,codec_name,codec_tag_string", "-of", "json", playlist].map(String::from).to_vec()
}

fn frames_per_second(rate: &str) -> String {
    let Some((numerator, denominator)) = rate.split_once('/') else { return String::new() };
    let (Ok(numerator), Ok(denominator)) = (numerator.parse::<f64>(), denominator.parse::<f64>()) else { return String::new() };
    if denominator == 0.0 {
        return String::new();
    }
    let fps = numerator / denominator;
    if fps <= 1.0 {
        String::new()
    } else if (fps - fps.round()).abs() < 0.05 {
        format!("@{:.0}", fps)
    } else {
        format!("@{fps:.2}")
    }
}

pub fn parse_picture(json: &str) -> Option<Picture> {
    let probe: Probe = serde_json::from_str(json).ok()?;
    let stream = probe.streams.into_iter().next()?;
    let (width, height) = (stream.width?, stream.height?);
    Some(Picture { resolution: format!("{width}x{height}{}", frames_per_second(&stream.avg_frame_rate)), codec: VideoCodec::named(&stream.codec_name), tag: stream.codec_tag_string })
}

impl Live {
    pub(crate) fn resolution_step(&self) {
        let finished = {
            let inner = self.lock();
            inner.resolution_probe.as_ref().is_some_and(|run| run.child.try_wait().ok().flatten().is_some() || run.started.elapsed() > GIVE_UP_AFTER)
        };
        if finished {
            self.finish_resolution();
            return;
        }
        let due = {
            let inner = self.lock();
            inner.resolution_probe.is_none() && inner.resolution.is_empty() && inner.ready && inner.phase == Phase::Running && inner.run.is_some() && inner.last_resolution.is_none_or(|at| at.elapsed() >= RETRY_EVERY)
        };
        if due {
            self.start_resolution();
        }
    }

    pub(crate) async fn retag_step(&self) {
        let channel = {
            let mut inner = self.lock();
            if !inner.retag || inner.phase != Phase::Running {
                return;
            }
            inner.retag = false;
            inner.current.clone()
        };
        let Some(channel) = channel else { return };
        let Ok(_gate) = self.gate.try_lock() else {
            self.lock().retag = true;
            return;
        };
        self.stop_run().await;
        if let Err(error) = self.start_run(&channel, true) {
            tracing::warn!(error = %error, "relaunch for the hvc1 tag failed");
            self.fail_start(&error);
        }
    }

    fn start_resolution(&self) {
        let Some(playlist) = self.lock().run.as_ref().map(|run| self.folder_path(&run.folder).join(PLAYLIST_FILE)) else { return };
        let Ok(child) = Command::new(self.programs.ffprobe.clone()).args(probe_args(&playlist.to_string_lossy())).spawn() else { return };
        let mut inner = self.lock();
        let generation = inner.generation;
        inner.last_resolution = Some(Instant::now());
        inner.resolution_probe = Some(ResolutionRun { child, generation, started: Instant::now() });
    }

    fn finish_resolution(&self) {
        let Some(run) = self.lock().resolution_probe.take() else { return };
        if run.child.try_wait().ok().flatten().is_none() {
            run.child.kill();
            return;
        }
        let (stdout, _) = drained(&run.child);
        let Some(picture) = parse_picture(&stdout) else { return };
        let mut inner = self.lock();
        if inner.generation != run.generation {
            return;
        }
        inner.resolution = picture.resolution;
        let Some(channel) = inner.current.clone() else { return };
        let already_tagged = picture.tag == HVC1;
        if picture.codec == VideoCodec::Hevc && !already_tagged && inner.codecs.get(&(channel.provider, channel.stream_id)) != Some(&VideoCodec::Hevc) {
            inner.retag = true;
        }
        if picture.codec == VideoCodec::Hevc {
            inner.codecs.insert((channel.provider, channel.stream_id), VideoCodec::Hevc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_is_width_height_and_a_rounded_frame_rate() {
        let json = r#"{"streams":[{"width":1920,"height":1080,"avg_frame_rate":"25/1","codec_name":"h264","codec_tag_string":"avc1"}]}"#;

        let picture = parse_picture(json).unwrap();

        assert_eq!((picture.resolution.as_str(), picture.codec, picture.tag.as_str()), ("1920x1080@25", VideoCodec::Other, "avc1"));
    }

    #[test]
    fn a_fractional_rate_keeps_two_decimals_and_an_unknown_one_is_left_out() {
        let json = |rate: &str| format!(r#"{{"streams":[{{"width":1280,"height":720,"avg_frame_rate":"{rate}","codec_name":"hevc","codec_tag_string":"hev1"}}]}}"#);

        assert_eq!(parse_picture(&json("51/2")).unwrap().resolution, "1280x720@25.50");
        assert_eq!(parse_picture(&json("0/0")).unwrap().resolution, "1280x720");
        assert_eq!(parse_picture(&json("1/1")).unwrap().resolution, "1280x720");
        assert_eq!(parse_picture(&json("50/1")).unwrap().codec, VideoCodec::Hevc);
    }

    #[test]
    fn a_probe_without_a_picture_says_nothing() {
        assert_eq!(parse_picture(r#"{"streams":[]}"#), None);
        assert_eq!(parse_picture(r#"{"streams":[{"codec_name":"aac"}]}"#), None);
        assert_eq!(parse_picture("not json"), None);
    }

    #[test]
    fn the_probe_reads_the_local_playlist_and_selects_the_first_video_stream() {
        let args = probe_args("/p/index.m3u8");

        assert_eq!(args[args.iter().position(|a| a == "-select_streams").unwrap() + 1], "v:0");
        assert_eq!(args[args.iter().position(|a| a == "-allowed_extensions").unwrap() + 1], "ALL");
        assert_eq!(args.last().map(String::as_str), Some("/p/index.m3u8"));
    }
}
