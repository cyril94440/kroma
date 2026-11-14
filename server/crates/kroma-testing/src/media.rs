use std::path::{Path, PathBuf};
use std::process::Command;

/// Five seconds of colour bars and a tone at `<dir>/clip.mp4`, keyframed every
/// second so a seek has somewhere to land. Needs `ffmpeg` on PATH, which every
/// job that runs these tests installs.
pub fn clip(dir: &Path) -> PathBuf {
    let at = dir.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc=size=320x180:rate=15:duration=5"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=5"])
        .args(["-c:v", "libx264", "-g", "15", "-pix_fmt", "yuv420p"])
        .args(["-c:a", "aac", "-shortest"])
        .arg(&at)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg could not write the fixture clip");
    at
}

/// Two seconds of HDR10 (10-bit HEVC tagged BT.2020 / PQ) and a tone at
/// `<dir>/hdr10.mp4`. Needs an `ffmpeg` with `libx265`.
pub fn hdr10_clip(dir: &Path) -> PathBuf {
    let at = dir.join("hdr10.mp4");
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-y"])
        .args(["-f", "lavfi", "-i", "testsrc=size=320x180:rate=15:duration=2"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
        .args(["-c:v", "libx265", "-pix_fmt", "yuv420p10le", "-tag:v", "hvc1"])
        .args([
            "-x265-params",
            "colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:log-level=error",
        ])
        .args(["-c:a", "aac", "-shortest"])
        .arg(&at)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg could not write the HDR10 fixture clip");
    at
}
