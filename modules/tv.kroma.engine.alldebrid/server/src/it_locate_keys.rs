use axum::http::StatusCode;
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use serde_json::{json, Value};

use crate::fake_alldebrid::Fake;
use crate::it_locate::{known_to, locate, movie, post};

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_rotated_key_keeps_older_magnets_working_in_either_order() {
    let script = movie();
    *script.valid_key.lock().unwrap() = Some("sekret-key-123".into());
    let valid = script.valid_key.clone();
    let fake = script.start();
    let host = StubHost::new();
    known_to(&host, &fake);

    *valid.lock().unwrap() = Some("rotated".into());
    let creds = crate::creds::Creds::new(&host.data_dir().join("alldebrid"));
    creds
        .remember(
            None,
            &crate::creds::Saved {
                url: fake.base.clone(),
                key: "rotated".into(),
            },
        )
        .unwrap();
    let after_rotation = locate(&host, "7:Movie.mkv", false).await;

    *valid.lock().unwrap() = Some("sekret-key-123".into());
    let rolled_back = locate(&host, "7:Movie.mkv", false).await;

    assert!(after_rotation["Ok"]["url"].is_string(), "{after_rotation}");
    assert!(rolled_back["Ok"]["url"].is_string(), "{rolled_back}");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_key_nobody_accepts_any_more_says_so_without_naming_it() {
    let script = movie();
    let valid = script.valid_key.clone();
    let fake = script.start();
    let host = StubHost::new();
    known_to(&host, &fake);
    *valid.lock().unwrap() = Some("someone-elses".into());

    let answer = locate(&host, "7:Movie.mkv", false).await;

    let message = answer["Err"].as_str().unwrap();
    assert!(message.contains("rejected the API key"), "{message}");
    assert!(!message.contains("sekret-key-123"));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn the_credentials_are_kept_in_the_modules_own_folder_and_never_in_a_stub() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let record = host.data_dir().join("alldebrid/credentials/7.json");

    let saved: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(saved["key"], "sekret-key-123");
    assert!(host
        .data_dir()
        .join("alldebrid/credentials/latest.json")
        .exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_reference_or_a_stranger_magnet_is_an_error_envelope() {
    let host = StubHost::new();

    let garbage = locate(&host, "nonsense", false).await;
    let unknown = locate(&host, "9:a.mkv", false).await;

    assert!(garbage["Err"].as_str().unwrap().contains("not a reference"));
    assert!(unknown["Err"]
        .as_str()
        .unwrap()
        .contains("no AllDebrid key"));
    let (status, _) = post(&host, "/_port/source/locate", json!({})).await;
    assert_eq!(status, StatusCode::OK, "every field defaults");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_magnet_alldebrid_no_longer_has_is_a_clear_error() {
    let script = movie();
    let fake = script.start();
    let host = StubHost::new();
    known_to(&host, &fake);
    let gone = Fake::start(|_| crate::fake_alldebrid::Resp::failure("MAGNET_INVALID_ID", "no"));
    let creds = crate::creds::Creds::new(&host.data_dir().join("alldebrid"));
    creds
        .remember(
            Some("7"),
            &crate::creds::Saved {
                url: gone.base.clone(),
                key: "k".into(),
            },
        )
        .unwrap();

    let answer = locate(&host, "7:Movie.mkv", false).await;

    assert!(
        answer["Err"]
            .as_str()
            .unwrap()
            .contains("no longer on AllDebrid"),
        "{answer}"
    );
}
