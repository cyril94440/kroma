use serde_json::{json, Value};
use std::path::Path;

use crate::fake_alldebrid::{scripted, Fake, Resp};
use crate::{AllDebrid, ClientDef};

pub(crate) const KEY: &str = "sekret-key-123";

pub(crate) fn engine(fake: &Fake, dir: &Path) -> AllDebrid {
    AllDebrid::new(
        &ClientDef {
            url: fake.base.clone(),
            username: String::new(),
            password: KEY.into(),
        },
        dir,
    )
    .unwrap()
}

fn node(name: &str, size: usize) -> Value {
    json!({ "n": name, "s": size, "l": format!("https://alldebrid.com/f/{name}") })
}

pub(crate) fn magnet(code: u8, extra: Value) -> Resp {
    let mut one = json!({
        "id": 7, "filename": "Some.Show.S01", "size": 1000, "status": "x", "statusCode": code,
        "downloaded": 500, "downloadSpeed": 99, "seeders": 4
    });
    one.as_object_mut()
        .unwrap()
        .extend(extra.as_object().cloned().unwrap_or_default());
    Resp::data(json!({ "magnets": [one] }))
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn test_reports_the_premium_account_and_sends_the_key_as_a_bearer() {
    let fake =
        Fake::start(|_| Resp::data(json!({ "user": { "username": "neo", "isPremium": true } })));
    let dir = kroma_module_sdk::testing::temp_dir("ad-user");

    let said = engine(&fake, dir.path()).test().unwrap();

    assert_eq!(said, "neo (premium)");
    let call = &fake.calls()[0];
    assert_eq!(call.path, "/v4/user");
    assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_free_account_is_refused_by_name() {
    let fake =
        Fake::start(|_| Resp::data(json!({ "user": { "username": "neo", "isPremium": false } })));
    let dir = kroma_module_sdk::testing::temp_dir("ad-free");

    let error = engine(&fake, dir.path()).test().unwrap_err().to_string();

    assert!(
        error.contains("not premium") && error.contains("neo"),
        "{error}"
    );
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_bad_key_is_named_without_echoing_it() {
    let fake = Fake::start(|_| Resp::failure("AUTH_BAD_APIKEY", "The apikey is invalid"));
    let dir = kroma_module_sdk::testing::temp_dir("ad-badkey");

    let error = engine(&fake, dir.path()).test().unwrap_err().to_string();

    assert!(error.contains("rejected the API key"), "{error}");
    assert!(!error.contains(KEY), "{error}");
}

#[test]
fn an_empty_key_fails_before_any_call() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-nokey");

    let error = AllDebrid::new(&ClientDef::default(), dir.path())
        .err()
        .unwrap()
        .to_string();

    assert!(error.contains("API key"), "{error}");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn something_that_is_not_the_api_is_not_mistaken_for_it() {
    let fake = Fake::start(|_| Resp::raw(502, b"<html>bad gateway</html>".to_vec()));
    let dir = kroma_module_sdk::testing::temp_dir("ad-html");

    let error = engine(&fake, dir.path()).test().unwrap_err().to_string();

    assert!(error.contains("HTTP 502"), "{error}");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_magnet_is_uploaded_and_its_id_is_the_client_ref() {
    let fake = Fake::start(|_| {
        Resp::data(json!({ "magnets": [{ "magnet": "m", "id": 4242, "ready": false }] }))
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-add");

    let id = engine(&fake, dir.path())
        .add("magnet:?xt=urn:btih:ABC&dn=x")
        .unwrap();

    assert_eq!(id, "4242");
    let call = &fake.calls_to("/v4/magnet/upload")[0];
    assert_eq!(call.form()["magnets[]"], "magnet:?xt=urn:btih:ABC&dn=x");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn an_upload_alldebrid_refuses_carries_its_reason() {
    let fake = Fake::start(|_| {
        Resp::data(
            json!({ "magnets": [{ "magnet": "m", "error": { "code": "MAGNET_MUST_BE_PREMIUM", "message": "premium only" } }] }),
        )
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-add-refused");

    let error = engine(&fake, dir.path())
        .add("magnet:?xt=urn:btih:ABC")
        .unwrap_err()
        .to_string();

    assert!(error.contains("MAGNET_MUST_BE_PREMIUM"), "{error}");
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_torrent_link_is_fetched_then_uploaded_as_a_file() {
    let torrent = b"d8:announce3:urle".to_vec();
    let served = torrent.clone();
    let fake = Fake::start(move |req| match req.path.as_str() {
        "/t.torrent" => Resp::raw(200, served.clone()),
        "/v4/magnet/upload/file" => Resp::data(json!({ "files": [{ "file": "t", "id": "9" }] })),
        _ => Resp::failure("NOPE", "x"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-torrent");

    let id = engine(&fake, dir.path())
        .add(&format!("{}/t.torrent", fake.base))
        .unwrap();

    assert_eq!(id, "9");
    let upload = &fake.calls_to("/v4/magnet/upload/file")[0];
    let body = String::from_utf8_lossy(&upload.body);
    assert!(body.contains("name=\"files[]\""), "{body}");
    assert!(body.contains("d8:announce3:urle"), "{body}");
    assert_eq!(
        std::fs::read_dir(dir.path().join("torrents"))
            .unwrap()
            .count(),
        0,
        "staged file removed"
    );
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_link_that_is_not_a_torrent_is_not_uploaded() {
    let fake = Fake::start(|_| Resp::raw(200, b"<html>login</html>".to_vec()));
    let dir = kroma_module_sdk::testing::temp_dir("ad-html-torrent");
    let alldebrid = engine(&fake, dir.path());

    let page = alldebrid
        .add(&format!("{}/t.torrent", fake.base))
        .unwrap_err()
        .to_string();
    let scheme = alldebrid.add("ftp://x/y").unwrap_err().to_string();

    assert!(page.contains("did not return a .torrent"), "{page}");
    assert!(scheme.contains("magnet link"), "{scheme}");
    assert!(fake.calls_to("/v4/magnet/upload/file").is_empty());
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn listing_files_flattens_and_sanitises_and_a_link_unlocks_afresh() {
    let tree = json!([{ "n": "..", "e": [node("ep1.mkv", 4), node("/etc/passwd", 2)] }]);
    let fake = scripted(tree, vec![("ep1.mkv", b"abcd".to_vec())], true);
    let dir = kroma_module_sdk::testing::temp_dir("ad-list");
    let alldebrid = engine(&fake, dir.path());

    let listed = alldebrid.list_files("7").unwrap();
    let url = alldebrid.direct_url("7", "_/ep1.mkv").unwrap();

    assert_eq!(
        listed.iter().map(|f| f.rel.as_str()).collect::<Vec<_>>(),
        ["_/ep1.mkv", "_/_etc_passwd"]
    );
    assert!(
        url.starts_with("http://127.0.0.1:") && url.ends_with("/dl/ep1.mkv"),
        "{url}"
    );
    assert!(alldebrid.direct_url("7", "nope").is_err());
}
