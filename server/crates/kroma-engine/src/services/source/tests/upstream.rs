use kroma_testing::RangedServer;

use super::*;

fn served(len: usize) -> (kroma_testing::TempDir, RangedServer, Vec<u8>, Arc<Sources>) {
    let dir = kroma_testing::temp_dir("sources-upstream");
    let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    let path = dir.path().join("a.bin");
    std::fs::write(&path, &bytes).unwrap();
    let server = RangedServer::serve(&path);
    let sources = harness(json!({ "Err": "unused" })).sources;
    (dir, server, bytes, sources)
}

#[tokio::test]
async fn a_range_is_read_from_a_public_https_name_through_the_production_checks() {
    let (_dir, server, bytes, sources) = served(10_000);
    let url = upstream(&public_name(&server));

    let body = sources.relay(&url, 100, 199, 10_000).await.unwrap();

    assert_eq!(read_all(body).await.unwrap(), &bytes[100..200]);
    assert_eq!(server.heard()[0].range.as_deref(), Some("bytes=100-199"));
}

#[tokio::test]
async fn a_redirect_is_not_followed_and_names_a_status_without_the_link() {
    let (_dir, server, _, sources) = served(100);
    let elsewhere = RangedServer::serve(&_dir.path().join("a.bin"));
    server.redirect_to(&format!("http://127.0.0.1:{}/private", elsewhere.port()));
    let url = upstream(&public_name(&server));

    let error = sources.relay(&url, 0, 9, 100).await.err().expect("a redirect is an error");

    assert!(error.to_string().contains("302"), "{error}");
    assert!(!error.to_string().contains("hunter2"));
    assert!(elsewhere.heard().is_empty(), "the redirect target was never contacted");
}

#[tokio::test]
async fn a_name_that_resolves_to_a_private_address_is_never_connected_to() {
    let (_dir, server, _, sources) = served(100);
    let host = "private-name.example.com";
    kroma_http::test_hosts::map_host(
        host,
        "10.0.0.5".parse().unwrap(),
        std::net::SocketAddr::from(([127, 0, 0, 1], server.port())),
    );

    let error = sources.relay(&upstream(host), 0, 9, 100).await.err().expect("refused");

    assert!(error.to_string().contains("not a public internet address"), "{error}");
    assert!(server.heard().is_empty());
}

#[tokio::test]
async fn an_upstream_that_reports_another_total_is_a_mismatch() {
    let (_dir, server, _, sources) = served(1_000);
    server.announce_total(5_000);
    let url = upstream(&public_name(&server));

    let error = sources.relay(&url, 0, 99, 1_000).await.err().expect("a mismatch");

    assert!(error.to_string().contains("another size"), "{error}");
}

#[tokio::test]
async fn a_file_whose_size_is_not_the_stubs_is_a_mismatch_even_when_it_answers_the_range() {
    let (_dir, server, _, sources) = served(1_000);
    let url = upstream(&public_name(&server));

    let error = sources.relay(&url, 0, 99, 2_000).await.err().expect("a mismatch");

    assert!(error.to_string().contains("another size"), "{error}");
}

#[tokio::test]
async fn an_upstream_that_ignores_ranges_serves_only_the_bytes_asked_for_from_the_start() {
    let (_dir, server, bytes, sources) = served(10_000);
    server.ignore_ranges();
    let url = upstream(&public_name(&server));

    let body = sources.relay(&url, 0, 99, 10_000).await.unwrap();

    assert_eq!(read_all(body).await.unwrap(), &bytes[..100]);
}

#[tokio::test]
async fn an_upstream_that_ignores_ranges_cannot_serve_a_later_one() {
    let (_dir, server, _, sources) = served(10_000);
    server.ignore_ranges();
    let url = upstream(&public_name(&server));

    assert!(sources.relay(&url, 500, 599, 10_000).await.is_err());
}

#[tokio::test]
async fn an_error_status_marks_the_link_stale_only_for_the_statuses_that_mean_it() {
    let (_dir, server, _, sources) = served(100);
    let url = upstream(&public_name(&server));

    let mut stale = Vec::new();
    for status in [401u16, 403, 404, 410, 500, 503] {
        server.fail_with(status);
        let error = sources.relay(&url, 0, 9, 100).await.err().unwrap();
        stale.push(link_is_stale(&error));
    }

    assert_eq!(stale, [true, true, true, true, false, false]);
}

#[tokio::test]
async fn a_host_that_drops_the_connection_ends_the_body_short_of_what_was_promised() {
    let (_dir, server, bytes, sources) = served(1_000_000);
    server.cut_bodies_after(100_000);
    let url = upstream(&public_name(&server));

    let body = sources.relay(&url, 0, 999_999, 1_000_000).await.unwrap();
    let got = read_all(body).await;

    let got = got.unwrap_or_default();
    assert!(!got.is_empty() && got.len() < bytes.len(), "{} bytes", got.len());
    assert_eq!(got, &bytes[..got.len()], "what did arrive is the start of the file");
}
