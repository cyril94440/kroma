use super::*;
use crate::ingest::test_support::*;
use kroma_domain::SpatialAudio;
use crate::sync_all;
use crate::testing::TempPool;
use std::collections::HashMap;

fn pool_with_probed_movie() -> TempPool {
    let p = pool();
    sync_all(
        &p,
        &[lib("lib")],
        &[],
        &[movie(
            "m1",
            "Dune",
            "lib",
            vec![file("f1", "/media/m1.mkv", false)],
        )],
        &HashMap::new(),
    )
    .unwrap();
    p
}

fn one_ms() -> FileProbe<'static> {
    FileProbe {
        duration_ms: Some(1),
        ..FileProbe::default()
    }
}

fn broken() -> FileProbe<'static> {
    FileProbe {
        unreadable: Some("Invalid data found when processing input"),
        ..FileProbe::default()
    }
}

#[test]
fn a_refused_file_write_fails_the_probe_rather_than_reporting_success() {
    let p = pool_with_probed_movie();
    p.get()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER no_file_update BEFORE UPDATE ON files \
             BEGIN SELECT RAISE(ABORT, 'refused'); END",
        )
        .unwrap();

    assert!(set_file_probe(&p, "f1", &one_ms()).is_err());
}

#[test]
fn a_refused_item_write_fails_the_probe_that_triggered_the_recompute() {
    let p = pool_with_probed_movie();
    p.get()
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER no_item_update BEFORE UPDATE ON items \
             BEGIN SELECT RAISE(ABORT, 'refused'); END",
        )
        .unwrap();

    assert!(set_file_probe(&p, "f1", &one_ms()).is_err());
}

#[test]
fn a_probe_result_for_a_file_that_is_gone_is_recorded_against_no_item() {
    let p = pool_with_probed_movie();
    set_file_probe(&p, "no-such-file", &one_ms()).unwrap();

    let container: String = p
        .get()
        .unwrap()
        .query_row("SELECT container FROM items WHERE id='m1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(container, "mkv", "no item was recomputed");
}

#[test]
fn an_item_with_nothing_probed_yet_keeps_its_scanned_columns() {
    let p = pool_with_probed_movie();
    let conn = p.get().unwrap();
    recompute_item_representative(&conn, "m1").unwrap();

    let container: String = conn
        .query_row("SELECT container FROM items WHERE id='m1'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(container, "mkv");
}

#[test]
fn an_unreadable_file_never_represents_an_item_that_has_a_readable_one() {
    let p = pool();
    sync_all(
        &p,
        &[lib("lib")],
        &[],
        &[movie(
            "m1",
            "Dune",
            "lib",
            vec![
                file("f1", "/media/broken.mkv", false),
                file("f2", "/media/good.mkv", false),
            ],
        )],
        &HashMap::new(),
    )
    .unwrap();

    set_file_probe(&p, "f1", &broken()).unwrap();
    set_file_probe(
        &p,
        "f2",
        &FileProbe {
            duration_ms: Some(7_200_000),
            video: Some(&video()),
            ..FileProbe::default()
        },
    )
    .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();
    assert_eq!(item.default_file_id.as_deref(), Some("f2"));
    assert_eq!(item.duration_ms, Some(7_200_000));
    let fault = item.files.iter().find(|f| f.id == "f1").unwrap();
    assert_eq!(
        fault.unreadable.as_deref(),
        Some("Invalid data found when processing input")
    );
}

#[test]
fn a_file_that_opens_on_a_re_probe_clears_the_fault_it_carried() {
    let p = pool_with_probed_movie();
    set_file_probe(&p, "f1", &broken()).unwrap();
    set_file_probe(
        &p,
        "f1",
        &FileProbe {
            duration_ms: Some(100),
            video: Some(&video()),
            ..FileProbe::default()
        },
    )
    .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();
    assert!(item.files[0].unreadable.is_none());
    assert_eq!(item.default_file_id.as_deref(), Some("f1"));
}

fn dolby_vision() -> VideoStream {
    VideoStream {
        hdr: true,
        hdr_format: Some(HdrFormat::DolbyVision),
        dolby_vision_profile: Some(7),
        dolby_vision_level: Some(6),
        dolby_vision_compatibility: Some(6),
        dolby_vision_enhancement_layer: true,
        ..video()
    }
}

fn atmos() -> AudioStream {
    AudioStream {
        index: 0,
        codec: "truehd".into(),
        channels: Some(8),
        language: Some("eng".into()),
        title: None,
        default: true,
        region: None,
        profile: Some("Dolby TrueHD + Dolby Atmos".into()),
        layout: Some("7.1".into()),
        spatial: Some(SpatialAudio::Atmos),
    }
}

#[test]
fn the_dolby_vision_detail_and_the_audio_profile_come_back_as_they_were_probed() {
    let p = pool_with_probed_movie();
    let (video, audio) = (dolby_vision(), atmos());

    set_file_probe(
        &p,
        "f1",
        &FileProbe {
            duration_ms: Some(100),
            video: Some(&video),
            audio: Some(&audio),
            audio_tracks: std::slice::from_ref(&audio),
            ..FileProbe::default()
        },
    )
    .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();
    let read = item.files[0].video.as_ref().expect("the probed video");
    assert_eq!(read.hdr_format, Some(HdrFormat::DolbyVision));
    assert_eq!(read.dolby_vision_profile, Some(7));
    assert_eq!(read.dolby_vision_level, Some(6));
    assert_eq!(read.dolby_vision_compatibility, Some(6));
    assert!(read.dolby_vision_enhancement_layer);
    let track = &item.files[0].audio_tracks[0];
    assert_eq!(track.profile.as_deref(), Some("Dolby TrueHD + Dolby Atmos"));
    assert_eq!(track.layout.as_deref(), Some("7.1"));
    assert_eq!(track.spatial, Some(SpatialAudio::Atmos));
}

#[test]
fn hdr10_plus_is_stored_under_its_own_spelling() {
    let p = pool_with_probed_movie();
    let plus = VideoStream {
        hdr: true,
        hdr_format: Some(HdrFormat::Hdr10Plus),
        ..video()
    };

    set_file_probe(
        &p,
        "f1",
        &FileProbe {
            video: Some(&plus),
            ..FileProbe::default()
        },
    )
    .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();
    assert_eq!(
        item.files[0].video.as_ref().unwrap().hdr_format,
        Some(HdrFormat::Hdr10Plus)
    );
}

#[test]
fn a_re_probe_without_dolby_vision_clears_what_the_last_one_recorded() {
    let p = pool_with_probed_movie();
    let dolby = dolby_vision();
    set_file_probe(
        &p,
        "f1",
        &FileProbe {
            video: Some(&dolby),
            ..FileProbe::default()
        },
    )
    .unwrap();
    let plain = video();

    set_file_probe(
        &p,
        "f1",
        &FileProbe {
            video: Some(&plain),
            ..FileProbe::default()
        },
    )
    .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();
    let read = item.files[0].video.as_ref().unwrap();
    assert_eq!(read.dolby_vision_level, None);
    assert_eq!(read.dolby_vision_compatibility, None);
    assert!(!read.dolby_vision_enhancement_layer);
}

#[test]
fn a_track_stored_before_the_audio_facts_reads_with_none_of_them() {
    let p = pool_with_probed_movie();
    p.get()
        .unwrap()
        .execute(
            "UPDATE files SET probed=1, audio_tracks='[{\"index\":0,\"codec\":\"dts\",\"channels\":6,\"language\":null,\"default\":true}]' WHERE id='f1'",
            [],
        )
        .unwrap();

    let item = crate::get_item(&p, "m1").unwrap().unwrap();

    let track = &item.files[0].audio_tracks[0];
    assert!(track.profile.is_none() && track.layout.is_none() && track.spatial.is_none());
}
