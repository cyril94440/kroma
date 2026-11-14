use kroma_module_sdk::host::HostCtx;
use std::time::{Duration, Instant};

use serde_json::json;

use crate::it_live::{load, playlist_of, request, rig, rig_with, until, Rig};
use crate::runtime::{now_ms, Programs};
use crate::status::Phase;

async fn pump(rig: &Rig, limit: Duration, mut check: impl FnMut(&Rig) -> bool) -> bool {
    let end = Instant::now() + limit;
    while Instant::now() < end {
        rig.live.tick().await;
        if check(rig) {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    check(rig)
}

fn serving(rig: &Rig) -> bool {
    rig.live.status().playlist.is_some() && playlist_of(rig).is_some_and(|p| p.contains("seg0"))
}

async fn playing(rig: &Rig, id: &str) {
    let outcome = rig.live.switch(request(id)).await;
    assert_eq!(outcome.status, "switched", "{outcome:?}");
    assert!(pump(rig, Duration::from_secs(30), serving).await, "no segment appeared: {:?}", rig.live.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_switch_starts_ffmpeg_and_publishes_a_sliding_fmp4_playlist_with_program_date_time() {
    let rig = rig();
    load(&rig);
    let began = Instant::now();

    playing(&rig, "p:1").await;

    let startup = began.elapsed();
    let playlist = playlist_of(&rig).unwrap();
    for needed in ["#EXT-X-VERSION:7", "#EXT-X-INDEPENDENT-SEGMENTS", "#EXT-X-MAP:URI=\"init.mp4\"", "#EXT-X-PROGRAM-DATE-TIME:"] {
        assert!(playlist.contains(needed), "{needed} missing in {playlist}");
    }
    assert!(!playlist.contains("#EXT-X-ENDLIST"));
    let status = rig.live.status();
    assert_eq!(status.state, Phase::Running);
    assert_eq!(status.channel.as_ref().map(|c| c.id.as_str()), Some("p:1"));
    assert_eq!(status.now.as_ref().map(|n| n.title.as_str()), Some("Journal"));
    eprintln!("MEASURE startup_to_first_segment_ms={}", startup.as_millis());
    assert!(pump(&rig, Duration::from_secs(10), |r| r.live.status().health.source_kbps > 0.0).await, "no bitrate measured");
    assert_eq!(rig.live.status().provider, crate::model::ProviderId::Primary);
}

#[tokio::test(flavor = "multi_thread")]
async fn zapping_writes_a_fresh_folder_and_removes_the_old_one() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;
    let first = rig.live.status().playlist.unwrap();
    let began = Instant::now();

    playing(&rig, "p:3").await;

    eprintln!("MEASURE zap_ms={}", began.elapsed().as_millis());
    let second = rig.live.status().playlist.unwrap();
    assert_ne!(first.split('/').next(), second.split('/').next());
    assert!(!rig.live.public_dir().join(&first).exists(), "stale segments must not stay");
    assert_eq!(rig.live.public_dir().read_dir().unwrap().count(), 1);
    assert!(pump(&rig, Duration::from_secs(5), |r| crate::db::recent_zaps(r.live.pool()).map(|z| !z.is_empty()).unwrap_or(false)).await);
    let zap = crate::db::recent_zaps(rig.live.pool()).unwrap().pop().unwrap();
    assert!(zap.total_ms > 0 && zap.publication_ms > 0, "{zap:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn asking_for_the_channel_already_on_air_changes_nothing() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;
    let before = rig.live.status().playlist;

    let outcome = rig.live.switch(crate::runtime::SwitchRequest { skip_if_same: true, ..request("p:1") }).await;

    assert_eq!(outcome.status, "already");
    assert_eq!(rig.live.status().playlist, before);
    assert_eq!(rig.live.switch(request("p:999")).await.status, "error");
}

#[tokio::test(flavor = "multi_thread")]
async fn pausing_stops_ffmpeg_and_the_folder_and_resuming_restarts_the_same_channel() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;

    assert_eq!(rig.live.pause().await, "paused");
    assert_eq!(rig.live.pause().await, "already_paused");
    let paused = rig.live.status();
    assert!(paused.state == Phase::Paused && paused.playlist.is_none());
    assert_eq!(rig.live.public_dir().read_dir().unwrap().count(), 0);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let open = rig.primary.streams_open.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(open, 0, "the upstream connection must be closed in standby");

    let resumed = rig.live.resume().await;
    assert_eq!(resumed.status, "resumed");
    assert!(pump(&rig, Duration::from_secs(30), serving).await);
    assert_eq!(rig.live.status().channel.unwrap().id, "p:1");
    assert_eq!(rig.live.resume().await.status, "already_active");
}

#[tokio::test(flavor = "multi_thread")]
async fn without_viewers_the_stream_goes_to_standby_after_the_delay_and_a_return_resumes_it() {
    let rig = rig_with(Programs::default(), |h| h.with_config("standbySeconds", json!(5)));
    load(&rig);
    playing(&rig, "p:1").await;
    rig.live.lock().presence.beat("u1", "Ann", now_ms());

    assert!(!pump(&rig, Duration::from_secs(3), |r| r.live.status().state == Phase::Paused).await, "never cut in front of a viewer");
    rig.live.lock().presence.leave("u1", now_ms());
    assert!(pump(&rig, Duration::from_secs(4), |r| r.live.status().state == Phase::Running).await, "still inside the delay");
    assert!(pump(&rig, Duration::from_secs(8), |r| r.live.status().state == Phase::Paused).await, "standby after the delay");

    rig.live.lock().presence.beat("u2", "Bob", now_ms());
    assert!(pump(&rig, Duration::from_secs(40), |r| r.live.status().state == Phase::Running && serving(r)).await, "a viewer coming back resumes it");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dead_source_is_restarted_with_a_backoff_and_reports_a_credential_free_error() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;

    rig.primary.broken.store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(pump(&rig, Duration::from_secs(20), |r| r.live.status().last_error.is_some()).await, "{:?}", rig.live.status());
    let status = rig.live.status();
    let error = status.last_error.unwrap();
    assert!(!error.contains("TESTPASS42") && !error.contains("127.0.0.1"), "{error}");
    assert!(status.restarts >= 1);
    assert!(status.health.source_kbps == 0.0);
    rig.primary.broken.store(false, std::sync::atomic::Ordering::SeqCst);
    assert!(pump(&rig, Duration::from_secs(45), |r| serving(r) && r.live.status().last_error.is_none()).await, "it must come back by itself: {:?}", rig.live.status());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_target_that_serves_a_finite_clip_is_off_air_and_nothing_is_cut() {
    let dir = kroma_module_sdk::testing::temp_dir("live-fake-ffprobe");
    let script = dir.path().join("ffprobe");
    std::fs::write(&script, "#!/bin/sh\ncase \"$*\" in *6.ts*) echo '{\"format\":{\"duration\":\"601.5\"}}';; *) echo '{\"format\":{\"duration\":\"N/A\"}}';; esac\n").unwrap();
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let rig = rig_with(Programs { ffmpeg: "ffmpeg".into(), ffprobe: script.to_string_lossy().into_owned() }, |h| h);
    load(&rig);
    playing(&rig, "p:1").await;
    let before = rig.live.status().playlist;

    let outcome = rig.live.switch(request("p:6")).await;

    assert_eq!(outcome.status, "offair");
    assert_eq!(outcome.duration, Some(601.5));
    assert_eq!(outcome.restored.as_deref(), Some("FR: TF1 FHD"));
    let status = rig.live.status();
    assert_eq!((status.playlist, status.state), (before, Phase::Running));
    assert_eq!(status.off_air.map(|o| o.name), Some("FR: OFFAIR HD".to_string()));
    assert!(!rig.primary.streams.lock().unwrap().contains(&"6".to_string()), "the off-air channel was never opened for streaming");
    assert_eq!(rig.live.switch(request("p:2")).await.status, "switched");
    assert!(rig.live.status().off_air.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_backup_provider_is_read_with_one_connection_and_without_ffmpeg_reconnects() {
    let rig = rig();
    load(&rig);

    playing(&rig, "s:1").await;

    let status = rig.live.status();
    assert!(status.on_backup && status.backup_configured);
    assert_eq!(rig.backup.max_open.load(std::sync::atomic::Ordering::SeqCst), 1, "never two connections at once at the backup");
    assert!(rig.primary.streams.lock().unwrap().is_empty());
    assert!(until(Duration::from_secs(1), || rig.live.admission(crate::runtime::Use::Probe, crate::model::ProviderId::Backup).is_err()).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_of_the_module_resumes_the_saved_channel_and_standby_state() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:3").await;
    rig.live.shutdown();
    let host = rig.host.clone();
    let reborn = crate::runtime::Live::new(std::sync::Arc::new(host.clone()), rig.pool.clone(), host.data_dir().join("live"), Programs::default());
    reborn.providers.forget();

    reborn.boot().await;

    assert_eq!(reborn.status().channel.map(|c| c.id), Some("p:3".to_string()));
    assert!(reborn.status().state == Phase::Running);
    reborn.shutdown();
}
