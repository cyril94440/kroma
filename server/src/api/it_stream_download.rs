use std::path::{Path, PathBuf};
use std::process::Command;

use axum::http::StatusCode;
use kroma_media::input;
use serde_json::Value;

use crate::api::test_support::{raw_bytes, test_app, TestApp};

struct Movie {
    t: TestApp,
    item: String,
    _dir: kroma_testing::TempDir,
}

async fn hdr10_movie() -> Movie {
    let dir = kroma_testing::temp_dir("download-hdr");
    let real = kroma_testing::hdr10_clip(dir.path());
    let t = test_app();
    let library = t.state.config.data_dir.join("library");
    let folder = library.join("Bright Movie (2020)");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(&real, folder.join("Bright Movie (2020).mp4")).unwrap();
    let state = t.state.clone();
    let root = library.to_string_lossy().into_owned();
    let scanned = tokio::task::spawn_blocking(move || {
        let mut defs = crate::services::settings::library_defs(&state.settings, &state.config);
        defs.push(crate::services::settings::LibraryDef {
            id: "bright-lib".into(),
            name: "Bright".into(),
            kind: "movies".into(),
            folders: vec![root],
            auto_scan: false,
        });
        crate::services::settings::set_library_defs(&state.settings, &state.db, &defs);
        crate::services::scan::rescan_sync(&state)
    })
    .await
    .unwrap()
    .unwrap();
    let crate::services::scan::Scanned::Applied(data) = scanned else {
        panic!("the library produced no items");
    };
    let item = data
        .items
        .iter()
        .find(|i| i.title == "Bright Movie")
        .unwrap();
    let file = item.files[0].clone();
    let (state, item_id) = (t.state.clone(), item.id.clone());
    let abs = file.abs_path.unwrap();
    tokio::task::spawn_blocking(move || {
        kroma_engine::infra::probe::probe_one(
            &state.db,
            true,
            &state.sources,
            &state.events,
            &file.id,
            &abs,
            &item_id,
        )
    })
    .await
    .unwrap()
    .unwrap();
    Movie {
        t,
        item: item.id.clone(),
        _dir: dir,
    }
}

struct Picture {
    codec: String,
    pix_fmt: String,
    transfer: Option<String>,
}

fn picture_of(dir: &Path, body: &[u8]) -> Picture {
    let file: PathBuf = dir.join("downloaded.mp4");
    std::fs::write(&file, body).unwrap();
    let probed = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-of", "json"])
        .args(["-show_entries", "stream=codec_name,pix_fmt,color_transfer"])
        .args(input::media(&file.into()))
        .output()
        .expect("run ffprobe");
    let parsed: Value = serde_json::from_slice(&probed.stdout).unwrap();
    let stream = &parsed["streams"][0];
    let text = |key: &str| stream[key].as_str().map(str::to_string);
    Picture {
        codec: text("codec_name").unwrap(),
        pix_fmt: text("pix_fmt").unwrap(),
        transfer: text("color_transfer"),
    }
}

async fn download(movie: &Movie, query: &str) -> Picture {
    let uri = format!("/api/items/{}/download{query}", movie.item);
    let (status, _, body) = raw_bytes(&movie.t.app, "GET", &uri, Some(&movie.t.token), &[]).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    picture_of(movie._dir.path(), &body)
}

#[tokio::test]
async fn a_download_that_declares_no_display_keeps_the_hdr_picture_as_it_always_did() {
    let movie = hdr10_movie().await;

    let picture = download(&movie, "").await;

    assert_eq!(picture.codec, "hevc");
    assert_eq!(picture.pix_fmt, "yuv420p10le");
    assert_eq!(picture.transfer.as_deref(), Some("smpte2084"));
}

#[tokio::test]
async fn a_download_for_an_sdr_only_display_is_tone_mapped_to_8_bit_h264_tagged_bt709() {
    let movie = hdr10_movie().await;

    let picture = download(&movie, "?hdr=").await;

    assert_eq!(picture.codec, "h264");
    assert_eq!(picture.pix_fmt, "yuv420p");
    assert_eq!(picture.transfer.as_deref(), Some("bt709"));
}

#[tokio::test]
async fn a_download_for_a_display_showing_hdr10_copies_the_picture() {
    let movie = hdr10_movie().await;

    let picture = download(&movie, "?hdr=hdr10").await;

    assert_eq!(picture.codec, "hevc");
    assert_eq!(picture.transfer.as_deref(), Some("smpte2084"));
}

#[tokio::test]
async fn a_download_for_a_display_that_lacks_the_sources_format_is_tone_mapped() {
    let movie = hdr10_movie().await;

    let picture = download(&movie, "?hdr=hlg").await;

    assert_eq!(picture.codec, "h264");
    assert_eq!(picture.transfer.as_deref(), Some("bt709"));
}
