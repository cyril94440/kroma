use super::*;

use axum::http::StatusCode;
use kroma_module_sdk::host::testing::StubHost;
use serde_json::json;

use crate::fake_alldebrid::{Fake, Resp};

async fn call(path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let app = routes::<StubHost>().with_state(StubHost::new());
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn unreachable_client() -> serde_json::Value {
    json!({ "url": "http://127.0.0.1:1", "username": "", "password": "k" })
}

#[tokio::test]
async fn every_method_is_mounted_where_the_consumer_calls_it() {
    for method in ["test", "add", "pause", "resume", "reannounce", "remove"] {
        let (status, answer) = call(
            &format!("/_port/tv.kroma.torrents/client/{method}"),
            json!({ "client": unreachable_client(), "args": { "client_ref": "7", "magnet_or_url": "magnet:?xt=urn:btih:AB" } }),
        )
        .await;

        assert_eq!(status, StatusCode::OK, "{method}");
        if method == "reannounce" {
            assert_eq!(answer, json!({ "Ok": null }), "{method}");
        } else {
            assert!(answer["Err"].is_string(), "{method} lost its reason: {answer}");
        }
    }
}

#[tokio::test]
async fn a_status_that_could_not_be_asked_is_a_bad_gateway_not_a_vanished_magnet() {
    let (status, _) = call(
        "/_port/tv.kroma.torrents/client/status",
        json!({ "client": unreachable_client(), "args": { "client_ref": "7" } }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn status_answers_null_for_a_forgotten_magnet_and_the_pinned_keys_otherwise() {
    let fake = Fake::start(|req| match req.query["id"].as_str() {
        "1" => Resp::failure("MAGNET_INVALID_ID", "gone"),
        _ => Resp::data(json!({ "magnets": [{ "id": 2, "filename": "n", "size": 10, "statusCode": 4 }] })),
    });
    let client = json!({ "url": fake.base, "password": "k" });

    let (_, gone) = call("/_port/tv.kroma.torrents/client/status", json!({ "client": client, "args": { "client_ref": "1" } })).await;
    let (_, found) = call("/_port/tv.kroma.torrents/client/status", json!({ "client": client, "args": { "client_ref": "2" } })).await;

    assert!(gone.is_null());
    let mut keys: Vec<&str> = found.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "client_ref", "down_bps", "error", "files", "info_hash", "lifetime_downloaded_bytes",
            "lifetime_uploaded_bytes", "name", "peers", "peers_seen", "progress", "save_path",
            "size_bytes", "state", "up_bps"
        ]
    );
    assert_eq!(found["state"], "completed");
}

#[tokio::test]
async fn a_body_with_no_client_at_all_is_rejected_by_the_extractor() {
    let (status, _) = call("/_port/tv.kroma.torrents/client/test", json!({})).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[test]
fn an_add_and_a_remove_carry_their_arguments() {
    let call: Call = serde_json::from_value(json!({
        "client": { "password": "k" },
        "args": { "magnet_or_url": "magnet:?xt=urn:btih:AB", "client_ref": "7", "delete_data": true, "label": "kroma", "download_dir": "/x" },
    }))
    .unwrap();

    assert_eq!(call.client.password, "k");
    assert_eq!(call.args.client_ref, "7");
    assert!(call.args.delete_data);
}

#[test]
fn a_state_crosses_as_its_lowercase_word() {
    for (state, word) in [
        (crate::TorrentState::Queued, "queued"),
        (crate::TorrentState::Downloading, "downloading"),
        (crate::TorrentState::Completed, "completed"),
        (crate::TorrentState::Error, "error"),
    ] {
        let status = wire(crate::TorrentStatus {
            client_ref: "7".into(),
            name: String::new(),
            info_hash: None,
            progress: 0.0,
            state,
            down_bps: 0,
            up_bps: 0,
            peers: 0,
            peers_seen: 0,
            size_bytes: 0,
            save_path: None,
            files: Vec::new(),
            error: None,
            lifetime_downloaded_bytes: 0,
            lifetime_uploaded_bytes: 0,
        });

        assert_eq!(status.state, word);
    }
}
