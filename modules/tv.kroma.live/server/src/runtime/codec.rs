use super::Live;
use crate::model::Channel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    Hevc,
    Other,
}

impl VideoCodec {
    pub fn named(name: &str) -> VideoCodec {
        match name.trim().to_ascii_lowercase().as_str() {
            "hevc" | "h265" => VideoCodec::Hevc,
            _ => VideoCodec::Other,
        }
    }
}

impl Live {
    pub(crate) fn note_codec(&self, channel: &Channel, codec: VideoCodec) {
        self.lock().codecs.insert((channel.provider, channel.stream_id), codec);
    }

    pub(crate) fn is_hevc(&self, channel: &Channel) -> bool {
        self.lock().codecs.get(&(channel.provider, channel.stream_id)) == Some(&VideoCodec::Hevc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hevc_is_recognised_under_both_of_its_names() {
        assert_eq!(VideoCodec::named("hevc"), VideoCodec::Hevc);
        assert_eq!(VideoCodec::named(" H265 "), VideoCodec::Hevc);
        assert_eq!(VideoCodec::named("h264"), VideoCodec::Other);
        assert_eq!(VideoCodec::named(""), VideoCodec::Other);
    }
}
