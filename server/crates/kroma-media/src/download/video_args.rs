//! The picture half of the download command: copied with the flags that keep
//! its HDR, or re-encoded to the 8-bit H.264 every download target decodes.

use kroma_module_wire::VideoStream;

use crate::hls::{ToneMap, SDR_TAGS};
use crate::stream_copy;

pub(super) fn video_args(
    video: Option<&VideoStream>,
    copy: bool,
    tone_map: Option<ToneMap>,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if copy {
        args.extend(["-c:v".into(), "copy".into()]);
        args.extend(stream_copy::video_args(video).into_iter().map(String::from));
        if let Some(level) = stream_copy::strictness(video, None) {
            args.extend(["-strict".into(), level.into()]);
        }
        return args;
    }
    if let Some(map) = tone_map {
        args.extend(["-vf".into(), map.chain()]);
        args.extend(SDR_TAGS.map(String::from));
    }
    args.extend(
        ["-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p"]
            .map(String::from),
    );
    args
}

#[cfg(test)]
mod tests {
    use crate::hls::{Filters, ToneSource};

    use super::*;

    fn hevc(profile: Option<u32>) -> VideoStream {
        VideoStream {
            codec: "hevc".into(),
            width: Some(3840),
            height: Some(2160),
            hdr: true,
            bit_depth: Some(10),
            hdr_format: None,
            dolby_vision_profile: profile,
            dolby_vision_level: None,
            dolby_vision_compatibility: None,
            dolby_vision_enhancement_layer: false,
            color: None,
        }
    }

    fn map() -> ToneMap {
        ToneMap::resolve(
            ToneSource::Pq,
            Filters {
                zimg: true,
                placebo: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn copied_hevc_is_tagged_hvc1_as_it_always_was() {
        assert_eq!(
            video_args(Some(&hevc(None)), true, None),
            ["-c:v", "copy", "-tag:v", "hvc1"]
        );
    }

    #[test]
    fn copied_dolby_vision_keeps_its_box_and_takes_the_sample_entry_its_profile_needs() {
        assert_eq!(
            video_args(Some(&hevc(Some(5))), true, None),
            ["-c:v", "copy", "-tag:v", "dvh1", "-strict", "unofficial"]
        );
        assert_eq!(
            video_args(Some(&hevc(Some(8))), true, None),
            ["-c:v", "copy", "-tag:v", "hvc1", "-strict", "unofficial"]
        );
    }

    #[test]
    fn a_re_encode_of_hdr_is_tone_mapped_and_tagged_bt709() {
        let args = video_args(Some(&hevc(None)), false, Some(map())).join(" ");

        assert!(args.starts_with("-vf setparams="), "{args}");
        assert!(args.contains("-colorspace bt709 -color_primaries bt709 -color_trc bt709"));
        assert!(args.ends_with("-c:v libx264 -preset veryfast -crf 21 -pix_fmt yuv420p"));
    }

    #[test]
    fn a_re_encode_with_nothing_to_map_is_the_command_it_always_was() {
        assert_eq!(
            video_args(Some(&hevc(None)), false, None),
            ["-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p"]
        );
    }
}
