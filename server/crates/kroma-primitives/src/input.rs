//! The one way an ffmpeg or ffprobe command names its input.

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub use crate::remote_url::{RemoteUrl, RemoteUrlError};

const LOCAL_PROTOCOLS: &str = "file";
const HTTPS_PROTOCOLS: &str = "https,tls,tcp";
#[cfg(feature = "insecure-test-urls")]
const HTTP_PROTOCOLS: &str = "http,tcp";
const NETWORK_OPTIONS: [&str; 12] = [
    "-reconnect",
    "1",
    "-reconnect_streamed",
    "1",
    "-reconnect_on_network_error",
    "1",
    "-reconnect_delay_max",
    "8",
    "-multiple_requests",
    "1",
    "-rw_timeout",
    "30000000",
];

/// What an ffmpeg or ffprobe command reads: a file on this machine, or a
/// network URL the core vetted first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaInput {
    Local(PathBuf),
    Remote(RemoteUrl),
}

impl MediaInput {
    /// The file this input is, when it is a file on this machine.
    pub fn local(&self) -> Option<&std::path::Path> {
        match self {
            Self::Local(path) => Some(path),
            Self::Remote(_) => None,
        }
    }

    /// `text` with this input's own name replaced by what is safe to log: a
    /// path stays, a URL becomes its host.
    pub fn redact(&self, text: &str) -> String {
        match self {
            Self::Local(path) => text.replace(&format!("{}: ", path.display()), ""),
            Self::Remote(url) => url.redact(text),
        }
    }
}

impl From<PathBuf> for MediaInput {
    fn from(path: PathBuf) -> Self {
        Self::Local(path)
    }
}

/// The arguments that make `input` the input, for ffmpeg and ffprobe alike.
pub fn media(input: &MediaInput) -> Vec<OsString> {
    match input {
        MediaInput::Local(path) => file(path).into(),
        MediaInput::Remote(url) => remote(url),
    }
}

/// The arguments that make `url` the input. The whitelist holds the protocols an
/// `https` read needs and nothing else, so a playlist or a redirect that names
/// `file:`, `concat:`, `crypto:` or `data:` is refused, and the reconnect and
/// timeout options keep a dropped connection from stalling a session.
pub fn remote(url: &RemoteUrl) -> Vec<OsString> {
    #[cfg(feature = "insecure-test-urls")]
    let protocols = if url.https { HTTPS_PROTOCOLS } else { HTTP_PROTOCOLS };
    #[cfg(not(feature = "insecure-test-urls"))]
    let protocols = HTTPS_PROTOCOLS;
    let mut args: Vec<OsString> = vec!["-protocol_whitelist".into(), protocols.into()];
    args.extend(NETWORK_OPTIONS.iter().map(OsString::from));
    args.push("-i".into());
    args.push(url.expose().into());
    args
}

/// The arguments that make `path` the input, for ffmpeg and ffprobe alike. Only
/// the `file` protocol is whitelisted: a URL, or a playlist naming one, is refused.
pub fn file(path: impl AsRef<OsStr>) -> [OsString; 4] {
    [
        "-protocol_whitelist".into(),
        LOCAL_PROTOCOLS.into(),
        "-i".into(),
        path.as_ref().to_os_string(),
    ]
}

/// The arguments that make a lavfi graph (`testsrc=...`, `color=...`) the input,
/// under the same whitelist as [`file`].
pub fn generated(graph: &str) -> [OsString; 6] {
    [
        "-protocol_whitelist".into(),
        LOCAL_PROTOCOLS.into(),
        "-f".into(),
        "lavfi".into(),
        "-i".into(),
        graph.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(args: &[OsString]) -> Vec<String> {
        args.iter().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn a_remote_input_whitelists_https_and_nothing_that_reads_a_file() {
        let url = RemoteUrl::parse("https://cdn.example.com/a.mkv").unwrap();

        let args = words(&remote(&url));

        let at = args.iter().position(|a| a == "-protocol_whitelist").unwrap();
        assert_eq!(args[at + 1], "https,tls,tcp");
        for banned in ["file", "concat", "crypto", "data", "http,", "pipe", "subfile", "hls"] {
            assert!(!args[at + 1].split(',').any(|p| p == banned.trim_end_matches(',')), "{banned}");
        }
        assert_eq!(&args[args.len() - 2..], ["-i", "https://cdn.example.com/a.mkv"]);
        assert!(args.windows(2).any(|w| w == ["-reconnect_streamed", "1"]));
        assert!(args.windows(2).any(|w| w == ["-rw_timeout", "30000000"]));
    }

    #[test]
    fn a_local_input_keeps_the_file_only_whitelist() {
        let args = words(&media(&MediaInput::Local("/m/a.mkv".into())));

        assert_eq!(args, ["-protocol_whitelist", "file", "-i", "/m/a.mkv"]);
    }
}
