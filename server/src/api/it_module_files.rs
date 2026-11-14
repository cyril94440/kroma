use axum::http::StatusCode;

use crate::api::test_support::{
    installed_module, media_ticket_for, raw_bytes, seed_session, test_app, ticketed, TestApp,
};
use crate::model::Permission;

const ID: &str = "tv.kroma.shows";
const PLAYLIST: &str = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2.0,\nseg1.m4s\n";

fn publisher(t: &TestApp, id: &str, extra: &str, official: bool) -> std::path::PathBuf {
    installed_module(t, id, extra);
    let installed = t
        .state
        .config
        .data_dir
        .join(kroma_module_supervisor::INSTALLED_DIR)
        .join(id);
    std::fs::write(
        installed.join("origin.json"),
        format!(r#"{{ "kind": "registry", "installedAt": 0, "official": {official} }}"#),
    )
    .unwrap();
    let public = t.state.config.data_dir.join("showfiles/public");
    std::fs::create_dir_all(public.join("abc123")).unwrap();
    std::fs::write(public.join("abc123/index.m3u8"), PLAYLIST).unwrap();
    std::fs::write(public.join("abc123/init.mp4"), b"init").unwrap();
    std::fs::write(public.join("abc123/seg1.m4s"), b"0123456789").unwrap();
    std::fs::write(public.join("abc123/cover.jpg"), b"jpg").unwrap();
    std::fs::write(public.join("abc123/cap.vtt"), b"WEBVTT").unwrap();
    std::fs::write(public.join("abc123/old.ts"), b"ts").unwrap();
    std::fs::write(t.state.config.data_dir.join("showfiles/secret.txt"), b"secret").unwrap();
    public
}

const OPTED_IN: &str = r#", "dataDirs": ["showfiles"], "permissions": { "publish": ["files"] }"#;

fn viewer(t: &TestApp) -> (String, String) {
    let (_, token) = seed_session(&t.state, "viewer@test.dev", "viewer", &[Permission::Playback]);
    let ticket = media_ticket_for(&t.state, &token);
    (token, ticket)
}

async fn fetch(
    t: &TestApp,
    uri: &str,
    token: Option<&str>,
    headers: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    raw_bytes(&t.app, "GET", uri, token, headers).await
}

#[tokio::test]
async fn a_request_with_no_credential_is_refused() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);

    let (status, _, _) = fetch(&t, &format!("/api/module-files/{ID}/abc123/seg1.m4s"), None, &[]).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_bearer_and_a_ticket_each_reach_the_file() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, ticket) = viewer(&t);
    let uri = format!("/api/module-files/{ID}/abc123/seg1.m4s");

    let (by_bearer, _, body) = fetch(&t, &uri, Some(&token), &[]).await;
    let (by_ticket, _, _) = fetch(&t, &ticketed(&uri, &ticket), None, &[]).await;

    assert_eq!((by_bearer, by_ticket), (StatusCode::OK, StatusCode::OK));
    assert_eq!(body, b"0123456789");
}

#[tokio::test]
async fn an_account_without_playback_is_refused() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (_, token) = seed_session(&t.state, "guest@test.dev", "guest", &[]);

    let (status, _, _) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/seg1.m4s"), Some(&token), &[]).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_path_that_climbs_out_of_the_public_folder_is_refused() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    for path in [
        "..%2fsecret.txt",
        "abc123%2f..%2f..%2fsecret.txt",
        "%2e%2e%2fsecret.txt",
        "%2fetc%2fpasswd",
        "abc123%5c..%5csecret.txt",
    ] {
        let (status, _, body) =
            fetch(&t, &format!("/api/module-files/{ID}/{path}"), Some(&token), &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(!String::from_utf8_lossy(&body).contains("secret"), "{path}");
    }
}

#[tokio::test]
async fn a_link_that_leaves_the_public_folder_is_not_followed() {
    let t = test_app();
    let public = publisher(&t, ID, OPTED_IN, true);
    let outside = t.state.config.data_dir.join("showfiles/secret.txt");
    std::os::unix::fs::symlink(&outside, public.join("abc123/leak.m4s")).unwrap();
    std::os::unix::fs::symlink(outside.parent().unwrap(), public.join("escape")).unwrap();
    let (token, _) = viewer(&t);

    for path in ["abc123/leak.m4s", "escape/secret.txt"] {
        let (status, _, body) =
            fetch(&t, &format!("/api/module-files/{ID}/{path}"), Some(&token), &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(body.is_empty() || !String::from_utf8_lossy(&body).contains("secret"));
    }
}

#[tokio::test]
async fn a_folder_is_never_listed() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    for path in ["abc123", "abc123/"] {
        let (status, _, _) =
            fetch(&t, &format!("/api/module-files/{ID}/{path}"), Some(&token), &[]).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn a_module_the_official_registry_did_not_ship_publishes_nothing() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, false);
    let (token, _) = viewer(&t);

    let (status, _, _) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/seg1.m4s"), Some(&token), &[]).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_module_that_did_not_opt_in_publishes_nothing() {
    let t = test_app();
    publisher(&t, ID, r#", "dataDirs": ["showfiles"]"#, true);
    let (token, _) = viewer(&t);

    let (status, _, _) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/seg1.m4s"), Some(&token), &[]).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_range_is_answered_with_the_bytes_asked_for() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    let (status, headers, body) = fetch(
        &t,
        &format!("/api/module-files/{ID}/abc123/seg1.m4s"),
        Some(&token),
        &[("range", "bytes=2-5")],
    )
    .await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers["content-range"], "bytes 2-5/10");
    assert_eq!(body, b"2345");
}

#[tokio::test]
async fn each_kind_of_file_is_served_as_what_a_player_expects() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    for (file, mime) in [
        ("index.m3u8", "application/vnd.apple.mpegurl"),
        ("init.mp4", "video/mp4"),
        ("seg1.m4s", "video/mp4"),
        ("old.ts", "video/mp2t"),
        ("cap.vtt", "text/vtt; charset=utf-8"),
        ("cover.jpg", "image/jpeg"),
    ] {
        let (status, headers, _) =
            fetch(&t, &format!("/api/module-files/{ID}/abc123/{file}"), Some(&token), &[]).await;
        assert_eq!(status, StatusCode::OK, "{file}");
        assert_eq!(headers["content-type"], mime, "{file}");
        assert_eq!(headers["x-content-type-options"], "nosniff", "{file}");
    }
}

#[tokio::test]
async fn a_playlist_is_never_cached_and_a_segment_only_briefly() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    let (_, playlist, _) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/index.m3u8"), Some(&token), &[]).await;
    let (_, segment, _) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/seg1.m4s"), Some(&token), &[]).await;

    assert_eq!(playlist["cache-control"], "no-store");
    assert_eq!(segment["cache-control"], "private, max-age=30");
}

#[tokio::test]
async fn a_playlist_fetched_with_a_ticket_hands_the_ticket_to_every_uri_in_it() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (_, ticket) = viewer(&t);

    let (status, _, body) = fetch(
        &t,
        &ticketed(&format!("/api/module-files/{ID}/abc123/index.m3u8"), &ticket),
        None,
        &[],
    )
    .await;

    let text = String::from_utf8(body).unwrap();
    assert_eq!(status, StatusCode::OK);
    assert!(text.contains(&format!("URI=\"init.mp4?t={ticket}\"")), "{text}");
    assert!(text.contains(&format!("seg1.m4s?t={ticket}\n")), "{text}");
}

#[tokio::test]
async fn a_playlist_fetched_with_a_bearer_is_served_as_written() {
    let t = test_app();
    publisher(&t, ID, OPTED_IN, true);
    let (token, _) = viewer(&t);

    let (_, _, body) =
        fetch(&t, &format!("/api/module-files/{ID}/abc123/index.m3u8"), Some(&token), &[]).await;

    assert_eq!(String::from_utf8(body).unwrap(), PLAYLIST);
}
