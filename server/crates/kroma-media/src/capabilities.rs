//! What the ffmpeg on this host can do that a build is free to leave out, read
//! once from its own listings: reading `http`, and the AC-3 / E-AC-3 encoders.

use std::process::Command;
use std::sync::OnceLock;

/// The audio encoders a surround track can be re-encoded with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioTargets {
    pub ac3: bool,
    pub eac3: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub http_input: bool,
    pub audio: AudioTargets,
}

impl Capabilities {
    pub fn from_listings(protocols: &str, encoders: &str) -> Self {
        let input_protocols = protocols
            .lines()
            .skip_while(|line| line.trim() != "Input:")
            .skip(1)
            .take_while(|line| line.trim() != "Output:");
        let http_input = input_protocols.map(str::trim).any(|name| name == "http");
        let has_encoder = |wanted: &str| {
            encoders
                .lines()
                .filter_map(|line| line.split_whitespace().nth(1))
                .any(|name| name == wanted)
        };
        Self {
            http_input,
            audio: AudioTargets {
                ac3: has_encoder("ac3"),
                eac3: has_encoder("eac3"),
            },
        }
    }
}

fn listing(flag: &str) -> String {
    Command::new("ffmpeg")
        .args(["-hide_banner", flag])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

fn detected() -> &'static Capabilities {
    static CAPS: OnceLock<Capabilities> = OnceLock::new();
    CAPS.get_or_init(|| Capabilities::from_listings(&listing("-protocols"), &listing("-encoders")))
}

/// Reads the listings now so the first request does not pay for them.
pub fn prime() {
    detected();
}

/// Whether ffmpeg and ffprobe here can read a title over the loopback relay. A
/// build without it reads a remote title only once its local copy exists.
pub fn network_input() -> bool {
    detected().http_input
}

pub fn audio_targets() -> AudioTargets {
    detected().audio
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROTOCOLS: &str =
        "Supported file protocols:\nInput:\n  file\n  http\n  tcp\nOutput:\n  file\n  http\n";
    const ENCODERS: &str = "Encoders:\n V....D libx264              libx264 H.264\n A....D ac3                  ATSC A/52A (AC-3)\n A....D aac                  AAC\n";

    #[test]
    fn http_and_the_encoders_are_read_from_the_listings() {
        let caps = Capabilities::from_listings(PROTOCOLS, ENCODERS);

        assert!(caps.http_input);
        assert_eq!(
            caps.audio,
            AudioTargets {
                ac3: true,
                eac3: false
            }
        );
    }

    #[test]
    fn a_build_with_no_network_reports_none() {
        let caps = Capabilities::from_listings(
            "Supported file protocols:\nInput:\n  file\n  pipe\nOutput:\n  file\n",
            "",
        );

        assert!(!caps.http_input);
        assert_eq!(
            caps.audio,
            AudioTargets {
                ac3: false,
                eac3: false
            }
        );
    }

    #[test]
    fn an_output_only_http_is_not_an_input() {
        let caps = Capabilities::from_listings("Input:\n  file\nOutput:\n  http\n", "");

        assert!(!caps.http_input);
    }
}
