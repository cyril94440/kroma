//! The picture half of the download command: copied with the flags that keep
//! its HDR, or re-encoded to the 8-bit H.264 every download target decodes.

use kroma_module_wire::VideoStream;

use crate::hls::{tone_map_filter, ToneMap, SDR_TAGS};
use crate::stream_copy;

pub(super) fn video_args(
    video: Option<&VideoStream>,
    copy: bool,
    dolby: bool,
    tone_map: Option<ToneMap>,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if copy {
        args.extend(["-c:v".into(), "copy".into()]);
        args.extend(stream_copy::video_args(video, dolby).into_iter().map(String::from));
        if let Some(level) = stream_copy::strictness(video, dolby, None) {
            args.extend(["-strict".into(), level.into()]);
        }
        return args;
    }
    if let Some(map) = tone_map {
        let frame = video.and_then(|v| v.width.zip(v.height));
        args.extend(["-vf".into(), tone_map_filter(map, frame)]);
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
            video_args(Some(&hevc(None)), true, false, None),
            ["-c:v", "copy", "-tag:v", "hvc1"]
        );
    }

    #[test]
    fn copied_dolby_vision_keeps_its_box_and_takes_the_sample_entry_its_profile_needs() {
        assert_eq!(
            video_args(Some(&hevc(Some(5))), true, true, None),
            ["-c:v", "copy", "-tag:v", "dvh1", "-strict", "unofficial"]
        );
        assert_eq!(
            video_args(Some(&hevc(Some(8))), true, false, None),
            ["-c:v", "copy", "-tag:v", "hvc1", "-strict", "unofficial"]
        );
    }

    #[test]
    fn a_profile_5_copy_for_a_client_without_dolby_vision_is_plain_hevc_with_no_box() {
        assert_eq!(
            video_args(Some(&hevc(Some(5))), true, false, None),
            ["-c:v", "copy", "-tag:v", "hvc1"]
        );
    }

    #[test]
    fn a_re_encode_of_hdr_is_shrunk_to_1080p_before_it_is_tone_mapped_and_tagged_bt709() {
        let args = video_args(Some(&hevc(None)), false, false, Some(map())).join(" ");

        assert!(args.starts_with("-vf scale=w=1920:h=1080,setparams="), "{args}");
        assert!(args.contains("-colorspace bt709 -color_primaries bt709 -color_trc bt709"));
        assert!(args.ends_with("-c:v libx264 -preset veryfast -crf 21 -pix_fmt yuv420p"));
    }

    #[test]
    fn a_picture_inside_1080p_is_tone_mapped_where_it_is() {
        let mut small = hevc(None);
        small.width = Some(1920);
        small.height = Some(800);

        let args = video_args(Some(&small), false, false, Some(map())).join(" ");

        assert!(args.starts_with("-vf setparams="), "{args}");
    }

    #[test]
    fn a_re_encode_with_nothing_to_map_is_the_command_it_always_was() {
        assert_eq!(
            video_args(Some(&hevc(None)), false, false, None),
            ["-c:v", "libx264", "-preset", "veryfast", "-crf", "21", "-pix_fmt", "yuv420p"]
        );
    }
}
