//! The one way an ffmpeg or ffprobe command names its input.

use std::any::Any;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

pub use crate::remote_url::{RemoteUrl, RemoteUrlError};

const LOCAL_PROTOCOLS: &str = "file";
const RELAY_PROTOCOLS: &str = "http,tcp";
const RELAY_HOST: &str = "127.0.0.1";
const PROXY_VARIABLES: [&str; 14] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
    "ftp_proxy",
    "FTP_PROXY",
    "rtsp_proxy",
    "RTSP_PROXY",
    "socks_proxy",
    "SOCKS_PROXY",
];
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

/// A title read through the core's own loopback relay. It can name nothing but
/// a path on `127.0.0.1` built from two opaque tokens, so no URL a module
/// supplied can reach an ffmpeg command. The grant it carries is held for as
/// long as any clone of this value lives.
#[derive(Clone)]
pub struct Relayed {
    url: String,
    _grant: Arc<dyn Any + Send + Sync>,
}

impl Relayed {
    /// `None` when a token or the extension is not plain alphanumerics, which
    /// is all a token the relay mints ever is.
    pub fn new(
        port: u16,
        secret: &str,
        grant: &str,
        extension: &str,
        held: Arc<dyn Any + Send + Sync>,
    ) -> Option<Self> {
        let plain = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric());
        if !plain(secret) || !plain(grant) || !(extension.is_empty() || plain(extension)) {
            return None;
        }
        let name = if extension.is_empty() { "title".to_string() } else { format!("title.{extension}") };
        Some(Self {
            url: format!("http://{RELAY_HOST}:{port}/{secret}/{grant}/{name}"),
            _grant: held,
        })
    }

    fn expose(&self) -> &str {
        &self.url
    }

    /// `text` with this input's address replaced by the name safe to log.
    pub fn redact(&self, text: &str) -> String {
        text.replace(&self.url, "the relay")
    }
}

impl PartialEq for Relayed {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Eq for Relayed {}

impl fmt::Debug for Relayed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Relayed(127.0.0.1)")
    }
}

/// What an ffmpeg or ffprobe command reads: a file on this machine, or a title
/// the core relays to it over loopback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaInput {
    Local(PathBuf),
    Remote(Relayed),
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
    /// path stays, a relay address becomes a name.
    pub fn redact(&self, text: &str) -> String {
        match self {
            Self::Local(path) => text.replace(&format!("{}: ", path.display()), ""),
            Self::Remote(relayed) => relayed.redact(text),
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
        MediaInput::Remote(relayed) => relayed_args(relayed),
    }
}

fn relayed_args(relayed: &Relayed) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-protocol_whitelist".into(), RELAY_PROTOCOLS.into()];
    args.extend(NETWORK_OPTIONS.iter().map(OsString::from));
    args.push("-i".into());
    args.push(relayed.expose().into());
    args
}

/// Takes the proxy variables out of what `command` inherits. An ffmpeg that
/// reads the relay on loopback must not be routed through a proxy of the
/// environment: its reads would fail, or leave this machine.
pub fn without_proxies(command: &mut Command) -> &mut Command {
    for name in PROXY_VARIABLES {
        command.env_remove(name);
    }
    command
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

    fn relayed() -> Relayed {
        Relayed::new(4001, "s3cret", "grant1", "mkv", Arc::new(())).unwrap()
    }

    #[test]
    fn a_relayed_input_whitelists_http_and_tcp_and_nothing_else() {
        let args = words(&media(&MediaInput::Remote(relayed())));

        let at = args.iter().position(|a| a == "-protocol_whitelist").unwrap();
        assert_eq!(args[at + 1], "http,tcp");
        for banned in ["https", "tls", "file", "concat", "crypto", "data", "pipe", "subfile", "hls"] {
            assert!(!args[at + 1].split(',').any(|p| p == banned), "{banned}");
        }
        assert_eq!(&args[args.len() - 2..], ["-i", "http://127.0.0.1:4001/s3cret/grant1/title.mkv"]);
        assert!(args.windows(2).any(|w| w == ["-rw_timeout", "30000000"]));
    }

    #[test]
    fn a_relayed_input_cannot_name_another_host_or_path() {
        assert!(Relayed::new(1, "a/../b", "g", "mkv", Arc::new(())).is_none());
        assert!(Relayed::new(1, "s", "g@evil.example", "mkv", Arc::new(())).is_none());
        assert!(Relayed::new(1, "s", "g", "mkv?x", Arc::new(())).is_none());
        assert!(Relayed::new(1, "", "g", "mkv", Arc::new(())).is_none());
    }

    #[test]
    fn neither_debug_nor_redact_shows_the_secret_or_the_grant() {
        let input = MediaInput::Remote(relayed());

        let shown = format!("{input:?} {}", input.redact("http://127.0.0.1:4001/s3cret/grant1/title.mkv: Server returned 403"));

        assert!(!shown.contains("s3cret") && !shown.contains("grant1"), "{shown}");
    }

    #[test]
    fn every_proxy_variable_is_taken_off_a_command() {
        let mut command = Command::new("env");
        for name in ["http_proxy", "HTTPS_PROXY", "all_proxy", "NO_PROXY"] {
            command.env(name, "http://proxy.example:3128");
        }

        let out = without_proxies(&mut command).output().unwrap();

        let printed = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
        assert!(!printed.contains("proxy"), "{printed}");
    }

    #[test]
    fn a_local_input_keeps_the_file_only_whitelist() {
        let args = words(&media(&MediaInput::Local("/m/a.mkv".into())));

        assert_eq!(args, ["-protocol_whitelist", "file", "-i", "/m/a.mkv"]);
    }
}
