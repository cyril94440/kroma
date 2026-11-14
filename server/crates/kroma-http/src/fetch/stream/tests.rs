use kroma_testing::{temp_dir, RangedServer};
use tokio::io::AsyncReadExt;

use super::*;

fn served(len: usize) -> (kroma_testing::TempDir, RangedServer, Vec<u8>) {
    let dir = temp_dir("fetch-stream");
    let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    let path = dir.path().join("body.bin");
    std::fs::write(&path, &bytes).unwrap();
    let server = RangedServer::serve(&path);
    (dir, server, bytes)
}

#[tokio::test]
async fn a_body_is_read_as_it_arrives_with_its_status_and_headers() {
    let (_dir, server, bytes) = served(300_000);

    let mut streamed = Fetch::new().stream(&server.url("body.bin")).await.unwrap();
    let mut got = Vec::new();
    streamed.read_to_end(&mut got).await.unwrap();

    assert_eq!(streamed.status, 200);
    assert_eq!(streamed.header("content-length"), Some("300000"));
    assert_eq!(got, bytes);
}

#[tokio::test]
async fn a_range_header_picks_the_bytes_that_arrive() {
    let (_dir, server, bytes) = served(1_000);

    let mut streamed = Fetch::new()
        .header("range", "bytes=100-199")
        .stream(&server.url("body.bin"))
        .await
        .unwrap();
    let mut got = Vec::new();
    streamed.read_to_end(&mut got).await.unwrap();

    assert_eq!(streamed.status, 206);
    assert_eq!(streamed.header("content-range"), Some("bytes 100-199/1000"));
    assert_eq!(got, &bytes[100..200]);
}

#[tokio::test]
async fn an_error_status_is_an_answer_not_a_failure() {
    let (_dir, server, _) = served(10);
    server.fail_with(403);

    let streamed = Fetch::new().stream(&server.url("body.bin")).await.unwrap();

    assert_eq!(streamed.status, 403);
}

#[tokio::test]
async fn a_connection_that_never_opens_is_an_error_without_the_url() {
    let error = Fetch::new()
        .stream("http://127.0.0.1:1/secret/path?token=hunter2")
        .await
        .err()
        .expect("nothing listens there");

    assert!(!format!("{error:#}").contains("hunter2"), "{error:#}");
}

#[tokio::test]
async fn a_public_only_stream_never_reaches_a_loopback_address() {
    let (_dir, server, _) = served(10);

    let refused = Fetch::new()
        .public_only()
        .stream(&server.url("body.bin"))
        .await;

    assert!(refused.is_err());
    assert!(server.heard().is_empty(), "curl never connected");
}

#[tokio::test]
async fn a_redirect_is_never_followed_by_a_no_redirect_stream() {
    let (_dir, server, _) = served(10);
    server.fail_with(302);

    let streamed = Fetch::new()
        .no_redirects()
        .stream(&server.url("body.bin"))
        .await
        .unwrap();

    assert_eq!(streamed.status, 302);
    assert_eq!(server.heard().len(), 1);
}

#[cfg(target_os = "linux")]
fn curl_children() -> usize {
    let me = std::process::id().to_string();
    std::fs::read_dir("/proc")
        .unwrap()
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("stat")).ok())
        .filter(|stat| {
            let after_name = stat
                .rsplit_once(')')
                .map(|(_, rest)| rest.trim())
                .unwrap_or_default();
            let mut fields = after_name.split_whitespace();
            let (state, parent) = (fields.next(), fields.next());
            stat.contains("(curl)") && parent == Some(me.as_str()) && state != Some("Z")
        })
        .count()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn dropping_a_stream_ends_the_process_behind_it() {
    let (_dir, server, _) = served(40 * 1024 * 1024);
    let before = curl_children();

    let mut streamed = Fetch::new().stream(&server.url("body.bin")).await.unwrap();
    let mut first = [0u8; 1024];
    streamed.read_exact(&mut first).await.unwrap();
    assert!(
        curl_children() > before,
        "curl is running while the body is read"
    );
    drop(streamed);

    let mut left = curl_children();
    for _ in 0..50 {
        if left <= before {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        left = curl_children();
    }
    assert!(left <= before, "{left} curl processes left behind");
}
