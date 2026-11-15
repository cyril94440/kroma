#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceError {
    BadDevice,
    Full,
    RateLimited,
    NotJoined,
    Forbidden,
    PeerGone,
    SelfSignal,
    TooLarge,
    BadSignal,
}

impl VoiceError {
    pub fn code(self) -> &'static str {
        match self {
            VoiceError::BadDevice => "badDevice",
            VoiceError::Full => "full",
            VoiceError::RateLimited => "rateLimited",
            VoiceError::NotJoined => "notJoined",
            VoiceError::Forbidden => "forbidden",
            VoiceError::PeerGone => "peerGone",
            VoiceError::SelfSignal => "selfSignal",
            VoiceError::TooLarge => "tooLarge",
            VoiceError::BadSignal => "badSignal",
        }
    }
}
