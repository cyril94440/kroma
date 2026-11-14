use super::*;

fn silent_clip(dir: &std::path::Path) -> Option<PathBuf> {
    let path = dir.join("silent.mp4");
    std::process::Command::new("ffmpeg")
        .args(["-v", "error"])
        .args(input::generated("testsrc=duration=4:size=64x48:rate=25"))
        .args(["-c:v", "libx264", "-g", "25", "-an", "-y"])
        .arg(&path)
        .status()
        .ok()?
        .success()
        .then_some(path)
}

#[test]
fn the_audio_track_is_mapped_only_when_the_source_has_one() {
    let args = line(copy(AudioMode::Copy), None, None, software());

    assert!(args.contains("-map 0:a:0?"), "{args}");
}

#[test]
fn a_title_with_no_audio_track_still_cuts_a_playlist_in_every_audio_mode() {
    let dir = kroma_testing::temp_dir("hls-silent");
    let Some(clip) = silent_clip(dir.path()) else {
        return;
    };
    let input = MediaInput::Local(clip);
    for (index, mode) in [
        StreamMode::new(VideoMode::Copy, AudioMode::Copy),
        StreamMode::new(VideoMode::Copy, AudioMode::Aac),
        StreamMode::new(VideoMode::H264, AudioMode::Aac),
    ]
    .into_iter()
    .enumerate()
    {
        let out = dir.path().join(format!("out{index}"));
        std::fs::create_dir_all(&out).unwrap();
        let args = build(&Spec {
            input: &input,
            dir: &out,
            audio: 0,
            mode,
            start_secs: 0.0,
            burst: false,
            pipeline: software(),
            video: None,
            track: None,
        });

        let status = std::process::Command::new("ffmpeg").args(&args).status().unwrap();

        assert!(status.success(), "{mode:?}");
        assert!(out.join("index.m3u8").exists(), "{mode:?}");
    }
}
