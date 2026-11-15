use std::time::Duration;

use kroma_module_sdk::host::process::Command;
use serde::Deserialize;

use super::codec::VideoCodec;
use super::connections::Use;
use super::process::{drained, stop_child, wait_for_exit};
use super::Live;
use crate::model::Channel;
use crate::portal::redact;

pub const QUICK_LIMIT: Duration = Duration::from_secs(25);
pub const MAX_PLACEHOLDER_SECONDS: f64 = 14_400.0;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Probed {
    pub alive: bool,
    pub busy: bool,
    pub reason: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
}

impl Probed {
    pub fn off_air(&self) -> bool {
        self.duration.is_some_and(|d| d <= MAX_PLACEHOLDER_SECONDS)
    }

    fn refused(reason: &str) -> Probed {
        Probed { busy: true, reason: Some(reason.to_string()), ..Probed::default() }
    }

    fn failed(reason: impl Into<String>) -> Probed {
        Probed { reason: Some(reason.into()), ..Probed::default() }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Report {
    streams: Vec<Stream>,
    format: Format,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Stream {
    codec_type: String,
    codec_name: String,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: String,
    r_frame_rate: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Format {
    duration: Option<serde_json::Value>,
}

pub fn quick_args(url: &str, user_agent: &str) -> Vec<String> {
    ["-v", "error", "-user_agent", user_agent, "-print_format", "json", "-show_streams", "-show_format", url].map(String::from).to_vec()
}

fn rate(text: &str) -> Option<f64> {
    let (numerator, denominator) = text.split_once('/')?;
    let value = numerator.parse::<f64>().ok()? / denominator.parse::<f64>().ok().filter(|d| *d != 0.0)?;
    Some((value * 10.0).round() / 10.0)
}

pub fn parse_report(json: &str) -> Option<Probed> {
    let report: Report = serde_json::from_str(json).ok()?;
    let video = report.streams.iter().find(|s| s.codec_type == "video")?;
    let audio = report.streams.iter().find(|s| s.codec_type == "audio");
    let duration = report.format.duration.as_ref().and_then(|d| d.as_str().and_then(|s| s.trim().parse::<f64>().ok()).or_else(|| d.as_f64()));
    Some(Probed {
        alive: true,
        busy: false,
        reason: None,
        video_codec: Some(video.codec_name.to_ascii_uppercase()),
        audio_codec: audio.map(|a| a.codec_name.to_ascii_uppercase()),
        resolution: video.width.zip(video.height).map(|(w, h)| format!("{w}x{h}")),
        fps: rate(&video.avg_frame_rate).or_else(|| rate(&video.r_frame_rate)),
        duration,
    })
}

impl Live {
    pub(crate) async fn quick_probe(&self, channel: &Channel) -> Probed {
        if let Err(refusal) = self.admission(Use::Probe, channel.provider) {
            return Probed::refused(refusal.as_str());
        }
        let source = match self.source_for(channel) {
            Ok(source) => source,
            Err(error) => return Probed::failed(redact(&error.to_string())),
        };
        self.lock().tests_running += 1;
        let probed = self.run_quick(&source.url, source.user_agent.as_deref().unwrap_or("")).await;
        {
            let mut inner = self.lock();
            inner.tests_running = inner.tests_running.saturating_sub(1);
        }
        if probed.alive {
            self.note_codec(channel, VideoCodec::named(probed.video_codec.as_deref().unwrap_or("")));
        }
        probed
    }

    async fn run_quick(&self, url: &str, user_agent: &str) -> Probed {
        let spawned = Command::new(self.programs.ffprobe.clone()).args(quick_args(url, user_agent)).spawn();
        let child = match spawned {
            Ok(child) => child,
            Err(error) => return Probed::failed(redact(&error)),
        };
        if wait_for_exit(&child, QUICK_LIMIT).await.is_none() {
            stop_child(&child).await;
            return Probed::failed("timeout");
        }
        let (stdout, stderr) = drained(&child);
        parse_report(&stdout).unwrap_or_else(|| Probed::failed(stderr.lines().last().map(|l| l.chars().take(120).collect::<String>()).filter(|l| !l.is_empty()).unwrap_or_else(|| "noVideo".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"25/1"},{"codec_type":"audio","codec_name":"aac"}],"format":{"duration":"N/A"}}"#;

    #[test]
    fn a_report_with_a_video_stream_is_alive_with_its_codec_and_picture() {
        let probed = parse_report(REPORT).unwrap();

        assert!(probed.alive && !probed.off_air());
        assert_eq!((probed.video_codec.as_deref(), probed.audio_codec.as_deref(), probed.resolution.as_deref(), probed.fps), (Some("H264"), Some("AAC"), Some("1920x1080"), Some(25.0)));
    }

    #[test]
    fn a_finite_duration_means_a_placeholder_clip_and_a_very_long_one_means_live() {
        let finite = REPORT.replace("N/A", "601.5");
        let long = REPORT.replace("N/A", "86400.0");

        assert!(parse_report(&finite).unwrap().off_air());
        assert!(!parse_report(&long).unwrap().off_air());
    }

    #[test]
    fn audio_only_or_unreadable_output_is_not_a_picture() {
        assert_eq!(parse_report(r#"{"streams":[{"codec_type":"audio","codec_name":"aac"}]}"#), None);
        assert_eq!(parse_report("nope"), None);
    }

    #[test]
    fn the_probe_reads_streams_and_format_as_json() {
        let args = quick_args("http://x/1.ts", "UA");

        assert!(args.contains(&"-show_streams".to_string()) && args.contains(&"-show_format".to_string()));
        assert_eq!(args[args.iter().position(|a| a == "-user_agent").unwrap() + 1], "UA");
    }
}
