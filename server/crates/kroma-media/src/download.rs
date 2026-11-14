//! The ffmpeg command of an offline download: the whole title remuxed to one
//! fragmented MP4, copying what the client can play and re-encoding what it
//! cannot.

use std::ffi::OsString;

use kroma_module_wire::{AudioStream, VideoStream};

use crate::hls::{AudioMode, StreamMode, ToneMap, VideoMode};
use crate::input::{self, MediaInput};

mod audio_args;
mod video_args;

use audio_args::download_audio_args;
use video_args::video_args;

/// What a download is asked for and what the catalog knows of the title.
/// `client_video`, `client_copy` and `client_hdr` are the sets the client
/// declared: absent is undeclared, empty is none.
pub struct Request<'a> {
    pub input: &'a MediaInput,
    pub video: Option<&'a VideoStream>,
    pub audio: Option<&'a AudioStream>,
    pub audio_tracks: &'a [AudioStream],
    pub client_video: Option<&'a str>,
    pub client_copy: Option<&'a str>,
    pub client_hdr: Option<&'a str>,
}

/// Whether the picture is stream-copied: the client decodes the source codec,
/// and a source it cannot show in its dynamic range is not one that can be
/// tone-mapped instead.
pub fn video_is_copied(request: &Request<'_>, can_tone_map: bool) -> bool {
    let source = request.video.map(|v| v.codec.as_str());
    let decodes = match (request.client_video, source) {
        (None, _) => true,
        (Some(set), Some(codec)) => set.split(',').any(|c| c.trim().eq_ignore_ascii_case(codec)),
        (Some(_), None) => false,
    };
    decodes
        && StreamMode::new(VideoMode::Copy, AudioMode::Copy)
            .for_client_hdr(request.video, request.client_hdr, can_tone_map)
            .video
            .copies()
}

/// The arguments of the whole ffmpeg command, input to `pipe:1`.
pub fn args(request: &Request<'_>) -> Vec<OsString> {
    let tone_map = request.video.and_then(ToneMap::for_video);
    let copied = video_is_copied(request, tone_map.is_some());
    let mut args: Vec<OsString> = ["-v", "error", "-nostdin"].map(OsString::from).into();
    args.extend(input::media(request.input));
    args.extend(["-map", "0:v:0"].map(OsString::from));
    let dolby = request.client_hdr.is_some_and(|set| crate::hls::declares(set, "dv"));
    args.extend(
        video_args(request.video, copied, dolby, tone_map)
            .into_iter()
            .map(OsString::from),
    );
    args.extend(
        download_audio_args(
            request.audio_tracks,
            request.audio.map(|a| a.codec.as_str()),
            request.client_copy,
        )
        .into_iter()
        .map(OsString::from),
    );
    // NO `empty_moov`: the muxer cannot write an upfront moov for EAC3 (codec params
    // are only known after parsing packets); `frag_keyframe` emits it with the first
    // fragment instead, which a pipe handles fine.
    args.extend(
        ["-dn", "-map_chapters", "-1", "-movflags", "frag_keyframe+default_base_moof", "-f", "mp4", "pipe:1"]
            .map(OsString::from),
    );
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hevc_hdr10() -> VideoStream {
        VideoStream {
            codec: "hevc".into(),
            width: Some(3840),
            height: Some(2160),
            hdr: true,
            bit_depth: Some(10),
            hdr_format: Some(kroma_module_wire::HdrFormat::Hdr10),
            dolby_vision_profile: None,
            dolby_vision_level: None,
            dolby_vision_compatibility: None,
            dolby_vision_enhancement_layer: false,
            color: None,
        }
    }

    fn request<'a>(input: &'a MediaInput, video: Option<&'a VideoStream>) -> Request<'a> {
        Request {
            input,
            video,
            audio: None,
            audio_tracks: &[],
            client_video: None,
            client_copy: None,
            client_hdr: None,
        }
    }

    #[test]
    fn a_client_that_declares_nothing_gets_the_picture_copied() {
        let input = MediaInput::Local("/m/a.mkv".into());
        let video = hevc_hdr10();

        assert!(video_is_copied(&request(&input, Some(&video)), false));
    }

    #[test]
    fn a_codec_the_client_did_not_declare_is_re_encoded() {
        let input = MediaInput::Local("/m/a.mkv".into());
        let video = hevc_hdr10();
        let mut asked = request(&input, Some(&video));
        asked.client_video = Some("h264");

        assert!(!video_is_copied(&asked, false));
        asked.client_video = Some("h264, HEVC");
        assert!(video_is_copied(&asked, false));
    }

    #[test]
    fn an_hdr_picture_a_display_cannot_show_is_re_encoded_only_where_it_can_be_tone_mapped() {
        let input = MediaInput::Local("/m/a.mkv".into());
        let video = hevc_hdr10();
        let mut asked = request(&input, Some(&video));
        asked.client_hdr = Some("");

        assert!(!video_is_copied(&asked, true));
        assert!(video_is_copied(&asked, false));
    }

    #[test]
    fn the_command_names_the_input_the_maps_and_a_pipe_in_that_order() {
        let input = MediaInput::Local("/m/a.mkv".into());

        let said: Vec<String> = args(&request(&input, None))
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        let at = |needle: &str| said.iter().position(|a| a == needle).unwrap();
        assert!(at("/m/a.mkv") < at("0:v:0") && at("0:v:0") < at("pipe:1"));
        assert_eq!(said.last().map(String::as_str), Some("pipe:1"));
        assert!(said.contains(&"0:a:0?".to_string()), "{said:?}");
    }
}
