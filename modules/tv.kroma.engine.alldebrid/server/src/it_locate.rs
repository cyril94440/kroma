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

pub(crate) fn movie() -> Script {
    let body: Vec<u8> = (0..BODY_LEN).map(|i| (i % 251) as u8).collect();
    Script::new(
        json!([node("Movie.mkv", BODY_LEN)]),
        vec![("Movie.mkv", body)],
        true,
    )
}

pub(crate) async fn post(host: &StubHost, path: &str, body: Value) -> (StatusCode, Value) {
    let app = routes::<StubHost>().with_state(host.clone());
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let resp = tower::ServiceExt::oneshot(app, req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

pub(crate) async fn locate(host: &StubHost, reference: &str, warm: bool) -> Value {
    post(
        host,
        "/_port/source/locate",
        json!({ "ref": reference, "warm": warm }),
    )
    .await
    .1
}

pub(crate) fn known_to(host: &StubHost, fake: &Fake) {
    let alldebrid = engine(fake, &host.data_dir().join("alldebrid"));
    alldebrid.status("7").unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn an_uncached_file_answers_a_fresh_url_and_a_burst_unlocks_once() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let first = locate(&host, "7:Movie.mkv", false).await;
    let second = locate(&host, "7:Movie.mkv", false).await;

    let url = format!("{}/dl/Movie.mkv", fake.base);
    assert_eq!(
        first,
        json!({ "Ok": { "path": null, "url": url, "size": BODY_LEN } })
    );
    assert_eq!(first, second);
    assert_eq!(fake.calls_to("/v4/link/unlock").len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn warming_fills_the_cache_and_the_next_locate_answers_a_path() {
    let fake = movie().start();
    let host = StubHost::new();
    known_to(&host, &fake);

    let warming = locate(&host, "7:Movie.mkv", true).await;
    assert!(
        warming["Ok"]["url"].is_string(),
        "warm does not wait: {warming}"
    );
    let mut landed = Value::Null;
    for _ in 0..500 {
        landed = locate(&host, "7:Movie.mkv", false).await;
        if landed["Ok"]["path"].is_string() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let path = landed["Ok"]["path"]
        .as_str()
        .expect("the fill completed")
        .to_string();
    assert!(std::path::Path::new(&path).is_absolute());
    assert_eq!(std::fs::read(&path).unwrap().len(), BODY_LEN);
    assert_eq!(landed["Ok"]["url"], Value::Null);
    assert_eq!(landed["Ok"]["size"], BODY_LEN);
    let canonical = host
        .data_dir()
        .join("alldebrid/cache/7/Movie.mkv")
        .canonicalize()
        .unwrap();
    assert_eq!(path, canonical.to_string_lossy());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
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
    std::fs::File::options()
        .write(true)
        .open(&cached)
        .unwrap()
        .set_modified(old)
        .unwrap();

    let report = on_thread({
        let dir = host.data_dir().join("alldebrid");
        move || {
            Cache::new(&dir).purge(Limits {
                max_bytes: Some(0),
                max_age: Duration::ZERO,
            })
        }
    })
    .await;
    let after = locate(&host, "7:Movie.mkv", false).await;

    assert_eq!(report.removed, 1);
    assert!(
        after["Ok"]["path"].is_null() && after["Ok"]["url"].is_string(),
        "{after}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn with_the_cache_switched_off_a_warm_never_fetches() {
    let fake = movie().start();
    let host = StubHost::new().with_config("cacheMaxGb", json!(0));
    known_to(&host, &fake);

    let answer = locate(&host, "7:Movie.mkv", true).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(answer["Ok"]["url"].is_string());
    assert!(fake.calls_to("/dl/Movie.mkv").is_empty());
}
