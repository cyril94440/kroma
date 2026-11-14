//! One track inside a media file: its codec, and for video the HDR variant
//! and colour signalling a client is matched against.

use serde::{Deserialize, Serialize};

/// The HDR system a video stream carries. Absent is SDR. Each has its own client
/// support, so a set that renders HDR10 and a set that also renders Dolby Vision
/// are not the same set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HdrFormat {
    Hdr10,
    Hdr10Plus,
    DolbyVision,
    Hlg,
}

impl HdrFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            HdrFormat::Hdr10 => "hdr10",
            HdrFormat::Hdr10Plus => "hdr10Plus",
            HdrFormat::DolbyVision => "dolbyVision",
            HdrFormat::Hlg => "hlg",
        }
    }
    pub fn parse(s: &str) -> Option<HdrFormat> {
        match s {
            "hdr10" => Some(HdrFormat::Hdr10),
            "hdr10Plus" => Some(HdrFormat::Hdr10Plus),
            "dolbyVision" => Some(HdrFormat::DolbyVision),
            "hlg" => Some(HdrFormat::Hlg),
            _ => None,
        }
    }
}

/// Colour signalling as the container states it, in ffprobe's own spelling
/// (`bt2020`, `smpte2084`, `bt709`, ...) rather than a normalised one, because a
/// client matches the exact value and a rename would lose what was read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primaries: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<String>,
}

impl ColorInfo {
    pub fn is_empty(&self) -> bool {
        self.primaries.is_none() && self.transfer.is_none() && self.matrix.is_none()
    }
}

/// Video stream description (best-effort; fields may be null when unknown).
/// `hdr` is what a client that predates `hdr_format` reads: true for any variant,
/// and still true where the variant itself was never recorded, which is how a row
/// probed before the column existed keeps reading as HDR. So `hdr` without
/// `hdr_format` means "HDR, variant unknown", never SDR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoStream {
    pub codec: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub hdr: bool,
    #[serde(rename = "bitDepth")]
    pub bit_depth: Option<u32>,
    #[serde(rename = "hdrFormat", default, skip_serializing_if = "Option::is_none")]
    pub hdr_format: Option<HdrFormat>,
    #[serde(
        rename = "dolbyVisionProfile",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub dolby_vision_profile: Option<u32>,
    #[serde(
        rename = "dolbyVisionLevel",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub dolby_vision_level: Option<u8>,
    /// Dolby Vision's `bl_signal_compatibility_id`: 0 none (profile 5), 1 HDR10,
    /// 2 SDR, 4 HLG, 6 UHD Blu-ray HDR10.
    #[serde(
        rename = "dolbyVisionCompatibility",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub dolby_vision_compatibility: Option<u8>,
    #[serde(
        rename = "dolbyVisionEnhancementLayer",
        default,
        skip_serializing_if = "is_false"
    )]
    pub dolby_vision_enhancement_layer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorInfo>,
}

fn is_false(flag: &bool) -> bool {
    !*flag
}

/// The object-based layer an audio stream carries on top of its channel bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpatialAudio {
    Atmos,
    Dtsx,
}

/// One audio stream/track. An item can carry several (e.g. EN + FR, or a
/// director's commentary); `index` is the audio-relative position (0-based
/// among audio streams only), matching ffmpeg's `-map 0:a:<index>` selector
/// used when remuxing a chosen track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStream {
    #[serde(default)]
    pub index: u32,
    pub codec: String,
    pub channels: Option<u32>,
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial: Option<SpatialAudio>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleTrack {
    pub language: Option<String>,
    pub codec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub forced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hdr_variant_stores_under_the_spelling_it_travels_as() {
        for variant in [
            HdrFormat::Hdr10,
            HdrFormat::Hdr10Plus,
            HdrFormat::DolbyVision,
            HdrFormat::Hlg,
        ] {
            assert_eq!(
                serde_json::to_string(&variant).unwrap(),
                format!("\"{}\"", variant.as_str())
            );
            assert_eq!(HdrFormat::parse(variant.as_str()), Some(variant));
        }

        assert_eq!(HdrFormat::parse("hdr"), None);
    }

    #[test]
    fn a_stream_with_nothing_to_say_about_colour_is_empty() {
        let said_nothing = ColorInfo::default();
        let said_something = ColorInfo {
            transfer: Some("smpte2084".into()),
            ..ColorInfo::default()
        };

        assert!(said_nothing.is_empty());
        assert!(!said_something.is_empty());
    }

    #[test]
    fn a_stream_with_no_new_facts_serialises_exactly_as_before() {
        let video = VideoStream {
            codec: "hevc".into(),
            width: Some(3840),
            height: Some(2160),
            hdr: true,
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::Hdr10),
            dolby_vision_profile: None,
            dolby_vision_level: None,
            dolby_vision_compatibility: None,
            dolby_vision_enhancement_layer: false,
            color: None,
        };
        let audio = AudioStream {
            index: 0,
            codec: "eac3".into(),
            channels: Some(6),
            language: None,
            title: None,
            default: true,
            region: None,
            profile: None,
            layout: None,
            spatial: None,
        };

        let video = serde_json::to_string(&video).unwrap();
        let audio = serde_json::to_string(&audio).unwrap();

        assert!(!video.contains("dolbyVision"), "{video}");
        assert!(!audio.contains("profile") && !audio.contains("spatial"), "{audio}");
    }

    #[test]
    fn the_new_facts_travel_under_the_names_clients_read() {
        let video = VideoStream {
            codec: "hevc".into(),
            width: None,
            height: None,
            hdr: true,
            bit_depth: None,
            hdr_format: Some(HdrFormat::DolbyVision),
            dolby_vision_profile: Some(8),
            dolby_vision_level: Some(6),
            dolby_vision_compatibility: Some(1),
            dolby_vision_enhancement_layer: true,
            color: None,
        };
        let audio = AudioStream {
            index: 1,
            codec: "truehd".into(),
            channels: Some(8),
            language: None,
            title: None,
            default: false,
            region: None,
            profile: Some("Dolby TrueHD + Dolby Atmos".into()),
            layout: Some("7.1".into()),
            spatial: Some(SpatialAudio::Atmos),
        };

        let video = serde_json::to_value(&video).unwrap();
        let audio = serde_json::to_value(&audio).unwrap();

        assert_eq!(video["dolbyVisionLevel"], 6);
        assert_eq!(video["dolbyVisionCompatibility"], 1);
        assert_eq!(video["dolbyVisionEnhancementLayer"], true);
        assert_eq!(audio["profile"], "Dolby TrueHD + Dolby Atmos");
        assert_eq!(audio["layout"], "7.1");
        assert_eq!(audio["spatial"], "atmos");
        assert_eq!(serde_json::to_value(SpatialAudio::Dtsx).unwrap(), "dtsx");
    }

    #[test]
    fn a_track_stored_before_the_audio_facts_still_reads() {
        let track: AudioStream =
            serde_json::from_str(r#"{"index":0,"codec":"dts","channels":6,"language":null,"default":true}"#)
                .unwrap();

        assert!(track.profile.is_none() && track.layout.is_none() && track.spatial.is_none());
    }

    #[test]
    fn a_subtitle_track_stored_before_title_and_forced_still_reads() {
        let track: SubtitleTrack =
            serde_json::from_str(r#"{"language":"fre","codec":"subrip"}"#).unwrap();

        assert!(track.title.is_none());
        assert!(!track.forced);
        assert!(!serde_json::to_string(&track).unwrap().contains("title"));
    }
}
