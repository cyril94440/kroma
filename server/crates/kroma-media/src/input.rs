//! The one way an ffmpeg or ffprobe command names its input.

pub use kroma_primitives::input::{file, generated, media, without_proxies, MediaInput, Relayed};

#[cfg(test)]
mod tests;

/// An input reading a [`kroma_testing::RangedServer`] the way the relay would
/// serve it: a loopback address with two tokens in its path.
#[cfg(test)]
pub(crate) fn relayed_at(port: u16) -> MediaInput {
    let relayed = Relayed::new(port, "secret", "grant", "mp4", std::sync::Arc::new(()))
        .expect("plain tokens");
    MediaInput::Remote(relayed)
}
