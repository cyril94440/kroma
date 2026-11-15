use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use crate::cmd::{build, Options, Source};
use crate::it_live::{load, rig_with, until, Rig};
use crate::model::ProviderId;
use crate::runtime::Programs;

fn script(dir: &std::path::Path, body: &str) -> String {
    let path = dir.join("fake-ffmpeg");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_string_lossy().into_owned()
}

fn rig_printing(dir: &std::path::Path, body: &str) -> Rig {
    rig_with(Programs { ffmpeg: script(dir, body), ffprobe: "ffprobe".into() }, |h| h)
}

fn blocks(rig: &Rig) -> usize {
    rig.live.lock().health.blocks()
}

#[tokio::test(flavor = "multi_thread")]
async fn heavy_progress_output_is_read_within_a_budget_per_tick_and_no_line_is_lost() {
    let dir = kroma_module_sdk::testing::temp_dir("live-heavy");
    let lines = 120_000;
    std::fs::write(dir.path().join("progress.txt"), "out_time=00:00:01.000000\n".repeat(lines)).unwrap();
    let rig = rig_printing(dir.path(), &format!("cat {}/progress.txt\nexec sleep 60", dir.path().display()));
    load(&rig);
    let channel = rig.live.providers.find_by_ref("p:1").unwrap();
    rig.live.start_run(&channel, false).unwrap();
    let budget = crate::runtime::READ_BUDGET_BYTES / "out_time=00:00:01.000000\n".len() + 1;
    let (mut largest, mut previous) = (0, 0);

    let deadline = Instant::now() + Duration::from_secs(30);
    while blocks(&rig) < lines && Instant::now() < deadline {
        rig.live.tick().await;
        largest = largest.max(blocks(&rig) - previous);
        previous = blocks(&rig);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert_eq!(blocks(&rig), lines, "no line lost");
    assert!(largest <= budget, "one tick read {largest} lines, over the {budget} its budget allows");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_value_line_split_across_two_reads_is_counted_once_and_whole() {
    let dir = kroma_module_sdk::testing::temp_dir("live-split");
    let rig = rig_printing(dir.path(), "printf 'out_time=00:0'\nsleep 2\nprintf '0:02.000000\\nout_time=00:00:03.000000\\n'\nexec sleep 60");
    load(&rig);
    let channel = rig.live.providers.find_by_ref("p:1").unwrap();
    rig.live.start_run(&channel, false).unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    rig.live.tick().await;
    assert_eq!(blocks(&rig), 0, "a half line is not a block");
    assert!(until(Duration::from_secs(10), || {
        let live = rig.live.clone();
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(live.tick()));
        blocks(&rig) == 2
    })
    .await);

    assert_eq!(blocks(&rig), 2);
    rig.live.shutdown();
}

fn hevc_source(path: &str, hevc: bool) -> Source {
    Source { name: "h".into(), url: path.into(), provider: Some(ProviderId::Primary), user_agent: None, realtime: false, audio: None, hevc }
}

fn init_tag(folder: &std::path::Path) -> String {
    let out = std::process::Command::new("ffprobe").args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=codec_name,codec_tag_string", "-of", "csv=p=0"]).arg(folder.join("init.mp4")).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn has_libx265() -> bool {
    let out = std::process::Command::new("ffmpeg").args(["-hide_banner", "-encoders"]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains("libx265")
}

fn hevc_clip(dir: &std::path::Path) -> String {
    let path = dir.join("hevc.ts").to_string_lossy().into_owned();
    let made = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=25", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", "4", "-c:v", "libx265", "-preset", "ultrafast", "-x265-params", "keyint=25:min-keyint=25:log-level=error", "-pix_fmt", "yuv420p", "-c:a", "aac", "-f", "mpegts", "-y", &path])
        .status()
        .unwrap();
    assert!(made.success());
    path
}

fn publish(source: &Source, folder: &std::path::Path) {
    let args = build(source, &Options::default(), &folder.to_string_lossy());
    let status = std::process::Command::new("ffmpeg").args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().unwrap();
    assert!(status.success());
}

#[test]
fn a_hevc_source_is_published_with_the_hvc1_tag_apple_players_need() {
    if !has_libx265() {
        eprintln!("SKIPPED: this ffmpeg has no libx265");
        return;
    }
    let dir = kroma_module_sdk::testing::temp_dir("live-hevc");
    let clip = hevc_clip(dir.path());
    let (tagged, plain) = (dir.path().join("tagged"), dir.path().join("plain"));
    std::fs::create_dir_all(&tagged).unwrap();
    std::fs::create_dir_all(&plain).unwrap();

    publish(&hevc_source(&clip, true), &tagged);
    publish(&hevc_source(&clip, false), &plain);

    assert_eq!(init_tag(&tagged), "hevc,hvc1");
    assert_eq!(init_tag(&plain), "hevc,hev1", "without the tag ffmpeg writes hev1, which Apple players refuse");
}

#[test]
fn an_h264_source_is_untouched_by_the_hevc_path() {
    let dir = kroma_module_sdk::testing::temp_dir("live-h264");
    let clip = dir.path().join("h264.ts").to_string_lossy().into_owned();
    let made = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=25", "-t", "3", "-c:v", "libx264", "-preset", "ultrafast", "-g", "25", "-pix_fmt", "yuv420p", "-f", "mpegts", "-y", &clip])
        .status()
        .unwrap();
    assert!(made.success());
    let folder = dir.path().join("out");
    std::fs::create_dir_all(&folder).unwrap();

    publish(&hevc_source(&clip, false), &folder);

    assert_eq!(init_tag(&folder), "h264,avc1");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_codec_learned_from_a_probe_makes_the_next_start_tag_the_stream() {
    let rig = crate::it_live::rig();
    load(&rig);
    let channel = rig.live.providers.find_by_ref("p:1").unwrap();
    assert!(!rig.live.source_for(&channel).unwrap().hevc);

    rig.live.note_codec(&channel, crate::runtime::VideoCodec::Hevc);

    assert!(rig.live.source_for(&channel).unwrap().hevc);
    assert!(!rig.live.source_for(&rig.live.providers.find_by_ref("p:2").unwrap()).unwrap().hevc);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relaunch_that_fails_after_the_stop_is_retried_by_the_restart_timer() {
    let rig = crate::it_live::rig();
    load(&rig);
    crate::it_stream::playing(&rig, "p:1").await;
    rig.live.stop_run().await;

    let message = rig.live.fail_start(&anyhow::anyhow!("start ffmpeg: boom"));

    {
        let inner = rig.live.lock();
        assert_eq!((inner.phase, inner.last_error.clone()), (crate::status::Phase::Error, Some(message)));
        assert!(inner.restart_at.is_some_and(|at| at > Instant::now()));
        assert!(inner.run.is_none());
    }
    rig.live.lock().restart_at = Some(Instant::now() - Duration::from_secs(1));

    rig.live.restart_if_due();

    let inner = rig.live.lock();
    assert_eq!(inner.phase, crate::status::Phase::Running);
    assert!(inner.run.is_some() && inner.restart_at.is_none());
    drop(inner);
    rig.live.shutdown();
}
