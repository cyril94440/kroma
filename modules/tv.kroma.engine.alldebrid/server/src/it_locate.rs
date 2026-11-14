use std::time::{Duration, SystemTime};

use axum::http::StatusCode;
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::HostCtx;
use serde_json::{json, Value};

use crate::cache::{Cache, Limits};
use crate::fake_alldebrid::{Fake, Script};
use crate::it_engine::engine;
use crate::it_ready::{node, on_thread};
use crate::port::routes;

const BODY_LEN: usize = 200_000;

fn movie() -> Script {
    let body: Vec<u8> = (0..BODY_LEN).map(|i| (i % 251) as u8).collect();
    Script::new(json!([node("Movie.mkv", BODY_LEN)]), vec![("Movie.mkv", body)], true)
}

async fn post(host: &StubHost, path: &str, body: Value) -> (StatusCode, Value) {
    let app = routes::<StubHost>().with_state(host.clone());
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn locate(host: &StubHost, reference: &str, warm: bool) -> Value {
    post(host, "/_port/source/locate", json!({ "ref": reference, "warm": warm })).await.1
}

fn known_to(host: &StubHost, fake: &Fake) {
    let alldebrid = engine(fake, &host.data_dir().join("alldebrid"));
    alldebrid.status("7").unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn an_uncached_file_answers_a_fresh_url_and_a_burst_unlocks_once() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let first = locate(&host, "7:Movie.mkv", false).await;
    let second = locate(&host, "7:Movie.mkv", false).await;

    let url = format!("{}/dl/Movie.mkv", fake.base);
    assert_eq!(first, json!({ "Ok": { "path": null, "url": url, "size": BODY_LEN } }));
    assert_eq!(first, second);
    assert_eq!(fake.calls_to("/v4/link/unlock").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn warming_fills_the_cache_and_the_next_locate_answers_a_path() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let warming = locate(&host, "7:Movie.mkv", true).await;
    assert!(warming["Ok"]["url"].is_string(), "warm does not wait: {warming}");
    let mut landed = Value::Null;
    for _ in 0..500 {
        landed = locate(&host, "7:Movie.mkv", false).await;
        if landed["Ok"]["path"].is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let path = landed["Ok"]["path"].as_str().expect("the fill completed").to_string();
    assert!(std::path::Path::new(&path).is_absolute());
    assert_eq!(std::fs::read(&path).unwrap().len(), BODY_LEN);
    assert_eq!(landed["Ok"]["url"], Value::Null);
    assert_eq!(landed["Ok"]["size"], BODY_LEN);
    let canonical = host.data_dir().join("alldebrid/cache/7/Movie.mkv").canonicalize().unwrap();
    assert_eq!(path, canonical.to_string_lossy());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn a_purged_file_goes_back_to_a_url() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);
    locate(&host, "7:Movie.mkv", true).await;
    for _ in 0..500 {
        if locate(&host, "7:Movie.mkv", false).await["Ok"]["path"].is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let cached = host.data_dir().join("alldebrid/cache/7/Movie.mkv");
    let old = SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&cached).unwrap().set_modified(old).unwrap();

    let report = on_thread({
        let dir = host.data_dir().join("alldebrid");
        move || Cache::new(&dir).purge(Limits { max_bytes: Some(0), max_age: Duration::ZERO })
    })
    .await;
    let after = locate(&host, "7:Movie.mkv", false).await;

    assert_eq!(report.removed, 1);
    assert!(after["Ok"]["path"].is_null() && after["Ok"]["url"].is_string(), "{after}");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn with_the_cache_switched_off_a_warm_never_fetches() {
    let fake = movie().start();
    let host = StubHost::new().with_config("cacheMaxGb", json!(0));
    known_to(&host, &fake);

    let answer = locate(&host, "7:Movie.mkv", true).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(answer["Ok"]["url"].is_string());
    assert!(fake.calls_to("/dl/Movie.mkv").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn a_rotated_key_keeps_older_magnets_working_in_either_order() {
    let script = movie();
    *script.valid_key.lock().unwrap() = Some("sekret-key-123".into());
    let valid = script.valid_key.clone();
    let fake = script.start();
    let host = StubHost::new();
    known_to(&host, &fake);

    *valid.lock().unwrap() = Some("rotated".into());
    let creds = crate::creds::Creds::new(&host.data_dir().join("alldebrid"));
    creds.remember(None, &crate::creds::Saved { url: fake.base.clone(), key: "rotated".into() }).unwrap();
    let after_rotation = locate(&host, "7:Movie.mkv", false).await;

    *valid.lock().unwrap() = Some("sekret-key-123".into());
    let rolled_back = locate(&host, "7:Movie.mkv", false).await;

    assert!(after_rotation["Ok"]["url"].is_string(), "{after_rotation}");
    assert!(rolled_back["Ok"]["url"].is_string(), "{rolled_back}");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
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
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn the_credentials_are_kept_in_the_modules_own_folder_and_never_in_a_stub() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let record = host.data_dir().join("alldebrid/credentials/7.json");

    let saved: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    assert_eq!(saved["key"], "sekret-key-123");
    assert!(host.data_dir().join("alldebrid/credentials/latest.json").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_reference_or_a_stranger_magnet_is_an_error_envelope() {
    let host = StubHost::new();

    let garbage = locate(&host, "nonsense", false).await;
    let unknown = locate(&host, "9:a.mkv", false).await;

    assert!(garbage["Err"].as_str().unwrap().contains("not a reference"));
    assert!(unknown["Err"].as_str().unwrap().contains("no AllDebrid key"));
    let (status, _) = post(&host, "/_port/source/locate", json!({})).await;
    assert_eq!(status, StatusCode::OK, "every field defaults");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
async fn a_magnet_alldebrid_no_longer_has_is_a_clear_error() {
    let script = movie();
    let fake = script.start();
    let host = StubHost::new();
    known_to(&host, &fake);
    let gone = Fake::start(|_| crate::fake_alldebrid::Resp::failure("MAGNET_INVALID_ID", "no"));
    let creds = crate::creds::Creds::new(&host.data_dir().join("alldebrid"));
    creds.remember(Some("7"), &crate::creds::Saved { url: gone.base.clone(), key: "k".into() }).unwrap();

    let answer = locate(&host, "7:Movie.mkv", false).await;

    assert!(answer["Err"].as_str().unwrap().contains("no longer on AllDebrid"), "{answer}");
}
