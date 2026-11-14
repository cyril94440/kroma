//! The boot probe settles the device, the tone-map support and the build's
//! capabilities together, and after it every answer is the host's own.

use std::time::{Duration, Instant};

use kroma_media::capabilities;
use kroma_media::hls::{prime_hwaccel, tonemap_filters, HlsEngine};

#[test]
fn the_boot_probe_settles_every_answer_and_stays_bounded() {
    let started = Instant::now();

    prime_hwaccel();

    assert!(
        started.elapsed() < Duration::from_secs(60),
        "{:?}",
        started.elapsed()
    );
    assert!(!HlsEngine::hardware().reason.contains("not finished"));
    assert_eq!(
        capabilities::settled().http_input,
        capabilities::network_input()
    );
    let listed = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(" zscale "))
        .unwrap_or(false);
    if !listed {
        assert!(!tonemap_filters().zimg);
    }
}
