use axum::http::StatusCode;
use serde_json::Value;

use crate::api::test_support::{
    get, media_ticket_for, probe, raw_bytes, stubbed_movie, test_app, test_app_with_source,
    FakeSource, Stubbed, TestApp,
};
use kroma_testing::TempDir;

const SOURCE_FOLDER: &str = "sources/tv.example.debrid";

struct Playground {
    t: TestApp,
    fake: FakeSource,
    stubbed: Stubbed,
    bytes: Vec<u8>,
    real: std::path::PathBuf,
    _dir: TempDir,
}

fn pseudo_random(len: usize) -> Vec<u8> {
    let mut state = 0x9e37_79b9_u32;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

async fn playground_over(real_of: impl FnOnce(&std::path::Path) -> std::path::PathBuf) -> Playground {
    let dir = kroma_testing::temp_dir("remote-stream");
    let real = real_of(dir.path());
    let bytes = std::fs::read(&real).unwrap();
    let (t, fake) = test_app_with_source();
    let stubbed = stubbed_movie(&t, &real, "Remote.Movie.2020.mp4").await;
    fake.answers_url(&stubbed.url("movie.mp4"), bytes.len() as u64);
    Playground { t, fake, stubbed, bytes, real, _dir: dir }
}

async fn bytes_playground() -> Playground {
    playground_over(|dir| {
        let path = dir.join("movie.mp4");
        std::fs::write(&path, pseudo_random(3_000_000)).unwrap();
        path
    })
    .await
}

async fn clip_playground() -> Playground {
    playground_over(kroma_testing::clip).await
}

impl Playground {
    fn stream(&self) -> String {
        format!("/api/items/{}/stream", self.stubbed.item)
    }

    async fn fetch(&self, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        raw_bytes(&self.t.app, "GET", uri, Some(&self.t.token), headers).await
    }

    fn held_copy(&self) -> std::path::PathBuf {
        let dir = self.t.state.config.data_dir.join(SOURCE_FOLDER);
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("Movie.mp4");
        std::fs::write(&copy, &self.bytes).unwrap();
        copy
    }
}

#[tokio::test]
async fn a_stub_in_the_library_is_a_title_with_the_real_container_size_and_a_remote_flag() {
    let p = bytes_playground().await;

    let (status, item) = get(&p.t.app, &format!("/api/items/{}", p.stubbed.item), Some(&p.t.token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(item["title"], "Remote Movie");
    let file = &item["files"][0];
    assert_eq!(file["container"], "mp4");
    assert_eq!(file["size"], p.bytes.len());
    assert_eq!(file["remote"], true);
}

#[tokio::test]
async fn a_title_on_the_disk_carries_no_remote_flag() {
    let t = test_app();
    let (_, items) = get(&t.app, "/api/items", Some(&t.token)).await;
    let id = items.as_array().and_then(|list| list.first()).map(|item| item["id"].as_str().unwrap().to_string());
    let (_, item) = get(&t.app, &format!("/api/items/{}", id.unwrap()), Some(&t.token)).await;

    let file = item["files"].get(0).cloned().unwrap_or(Value::Null);

    assert!(file.is_object(), "{item}");
    assert!(file.get("remote").is_none(), "{file}");
}

#[tokio::test]
async fn probing_a_stub_reads_the_streams_over_the_url() {
    let p = clip_playground().await;

    probe(&p.t, &p.stubbed).await.expect("the source answered");

    let (_, item) = get(&p.t.app, &format!("/api/items/{}", p.stubbed.item), Some(&p.t.token)).await;
    let file = &item["files"][0];
    assert_eq!(file["probed"], true);
    assert_eq!(file["video"]["codec"], "h264");
    assert!(file["durationMs"].as_u64().is_some_and(|ms| (4_000..6_000).contains(&ms)), "{file}");
    assert!(p.stubbed.cdn.heard().iter().any(|r| r.range.is_some()));
}

#[tokio::test]
async fn a_source_that_does_not_answer_leaves_the_file_to_a_later_probe() {
    let p = clip_playground().await;
    p.fake.answers_error("AllDebrid is down");

    let first = probe(&p.t, &p.stubbed).await;

    assert!(first.is_err());
    let (_, item) = get(&p.t.app, &format!("/api/items/{}", p.stubbed.item), Some(&p.t.token)).await;
    assert_eq!(item["files"][0]["probed"], false);
    assert!(item["files"][0].get("unreadable").is_none_or(Value::is_null));
    p.fake.answers_url(&p.stubbed.url("clip.mp4"), p.bytes.len() as u64);
    probe(&p.t, &p.stubbed).await.expect("the retry reads it");
    let (_, item) = get(&p.t.app, &format!("/api/items/{}", p.stubbed.item), Some(&p.t.token)).await;
    assert_eq!(item["files"][0]["probed"], true);
}

#[tokio::test]
async fn a_whole_stream_is_relayed_byte_for_byte_and_asks_the_source_to_warm() {
    let p = bytes_playground().await;

    let (status, headers, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-length"], p.bytes.len().to_string());
    assert_eq!(headers["accept-ranges"], "bytes");
    assert_eq!(headers["content-type"], "video/mp4");
    assert!(body == p.bytes, "the relayed bytes differ from the source file");
    let asked = p.fake.asked();
    assert_eq!(asked[0]["warm"], true);
    assert_eq!(asked[0]["ref"], "7:Remote.Movie.2020.mp4");
}

#[tokio::test]
async fn a_bounded_range_is_relayed_as_a_206_of_exactly_those_bytes() {
    let p = bytes_playground().await;

    let (status, headers, body) = p.fetch(&p.stream(), &[("range", "bytes=1000-1999")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers["content-range"], format!("bytes 1000-1999/{}", p.bytes.len()));
    assert_eq!(headers["content-length"], "1000");
    assert_eq!(body, &p.bytes[1000..2000]);
    assert!(p.stubbed.cdn.heard().iter().any(|r| r.range.as_deref() == Some("bytes=1000-1999")));
}

#[tokio::test]
async fn an_open_ended_range_runs_to_the_last_byte() {
    let p = bytes_playground().await;
    let from = p.bytes.len() - 5_000;

    let (status, headers, body) = p.fetch(&p.stream(), &[("range", &format!("bytes={from}-"))]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers["content-range"], format!("bytes {from}-{}/{}", p.bytes.len() - 1, p.bytes.len()));
    assert_eq!(body, &p.bytes[from..]);
}

#[tokio::test]
async fn a_range_past_the_end_is_a_416_that_never_reaches_the_host() {
    let p = bytes_playground().await;

    let (status, headers, _) = p.fetch(&p.stream(), &[("range", &format!("bytes={}-", p.bytes.len()))]).await;

    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(headers["content-range"], format!("bytes */{}", p.bytes.len()));
    assert!(p.stubbed.cdn.heard().is_empty());
}

#[tokio::test]
async fn a_head_answers_the_length_without_opening_the_host() {
    let p = bytes_playground().await;

    let (status, headers, body) = raw_bytes(&p.t.app, "HEAD", &p.stream(), Some(&p.t.token), &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-length"], p.bytes.len().to_string());
    assert!(body.is_empty());
    assert!(p.stubbed.cdn.heard().is_empty());
}

#[tokio::test]
async fn a_media_ticket_streams_it_like_a_bearer() {
    let p = bytes_playground().await;
    let ticket = media_ticket_for(&p.t.state, &p.t.token);

    let (status, _, body) = raw_bytes(&p.t.app, "GET", &format!("{}?t={ticket}", p.stream()), None, &[("range", "bytes=0-9")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, &p.bytes[..10]);
}

#[tokio::test]
async fn a_stream_with_no_credential_is_refused_before_the_source_is_asked() {
    let p = bytes_playground().await;

    let (status, _, _) = raw_bytes(&p.t.app, "GET", &p.stream(), None, &[]).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(p.fake.asked().is_empty());
}

#[tokio::test]
async fn a_host_that_drops_the_connection_ends_the_body_short_rather_than_forever() {
    let p = bytes_playground().await;
    p.stubbed.cdn.cut_bodies_after(100_000);

    let (status, _, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.len() >= 100_000, "everything the host sent before it dropped arrived: {}", body.len());
    assert!(body.len() < p.bytes.len(), "{} of {}", body.len(), p.bytes.len());
    assert_eq!(body, &p.bytes[..body.len()], "what arrived is the start of the file, in order");
}

#[tokio::test]
async fn a_host_that_refuses_the_range_is_a_translated_502() {
    let p = bytes_playground().await;
    p.stubbed.cdn.fail_with(403);

    let (status, _, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let said: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["code"], "error.sourceUnreachable");
    assert!(!said.to_string().contains("hunter2"));
}

mod derived;
mod sources;
