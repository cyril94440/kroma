use std::path::Path;
use serde_json::{json, Value};

use crate::fake_alldebrid::{scripted, Fake, Resp};
use crate::{AllDebrid, ClientDef, TorrentState};

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

fn magnet(code: u8, extra: Value) -> Resp {
    let mut one = json!({
        "id": 7, "filename": "Some.Show.S01", "size": 1000, "status": "x", "statusCode": code,
        "downloaded": 500, "downloadSpeed": 99, "seeders": 4
    });
    one.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
    Resp::data(json!({ "magnets": [one] }))
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn test_reports_the_premium_account_and_sends_the_key_as_a_bearer() {
    let fake = Fake::start(|_| Resp::data(json!({ "user": { "username": "neo", "isPremium": true } })));
    let dir = kroma_module_sdk::testing::temp_dir("ad-user");

    let said = engine(&fake, dir.path()).test().unwrap();

    assert_eq!(said, "neo (premium)");
    let call = &fake.calls()[0];
    assert_eq!(call.path, "/v4/user");
    assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn a_free_account_is_refused_by_name() {
    let fake = Fake::start(|_| Resp::data(json!({ "user": { "username": "neo", "isPremium": false } })));
    let dir = kroma_module_sdk::testing::temp_dir("ad-free");

    let error = engine(&fake, dir.path()).test().unwrap_err().to_string();

    assert!(error.contains("not premium") && error.contains("neo"), "{error}");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
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

    let error = AllDebrid::new(&ClientDef::default(), dir.path()).err().unwrap().to_string();

    assert!(error.contains("API key"), "{error}");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn something_that_is_not_the_api_is_not_mistaken_for_it() {
    let fake = Fake::start(|_| Resp::raw(502, b"<html>bad gateway</html>".to_vec()));
    let dir = kroma_module_sdk::testing::temp_dir("ad-html");

    let error = engine(&fake, dir.path()).test().unwrap_err().to_string();

    assert!(error.contains("HTTP 502"), "{error}");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn a_magnet_is_uploaded_and_its_id_is_the_client_ref() {
    let fake = Fake::start(|_| Resp::data(json!({ "magnets": [{ "magnet": "m", "id": 4242, "ready": false }] })));
    let dir = kroma_module_sdk::testing::temp_dir("ad-add");

    let id = engine(&fake, dir.path()).add("magnet:?xt=urn:btih:ABC&dn=x").unwrap();

    assert_eq!(id, "4242");
    let call = &fake.calls()[0];
    assert_eq!(call.path, "/v4/magnet/upload");
    assert_eq!(call.form()["magnets[]"], "magnet:?xt=urn:btih:ABC&dn=x");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn an_upload_alldebrid_refuses_carries_its_reason() {
    let fake = Fake::start(|_| {
        Resp::data(json!({ "magnets": [{ "magnet": "m", "error": { "code": "MAGNET_MUST_BE_PREMIUM", "message": "premium only" } }] }))
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-add-refused");

    let error = engine(&fake, dir.path()).add("magnet:?xt=urn:btih:ABC").unwrap_err().to_string();

    assert!(error.contains("MAGNET_MUST_BE_PREMIUM"), "{error}");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn a_torrent_link_is_fetched_then_uploaded_as_a_file() {
    let torrent = b"d8:announce3:urle".to_vec();
    let served = torrent.clone();
    let fake = Fake::start(move |req| match req.path.as_str() {
        "/t.torrent" => Resp::raw(200, served.clone()),
        "/v4/magnet/upload/file" => Resp::data(json!({ "files": [{ "file": "t", "id": "9" }] })),
        _ => Resp::failure("NOPE", "x"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-torrent");

    let id = engine(&fake, dir.path()).add(&format!("{}/t.torrent", fake.base)).unwrap();

    assert_eq!(id, "9");
    let upload = &fake.calls_to("/v4/magnet/upload/file")[0];
    let body = String::from_utf8_lossy(&upload.body);
    assert!(body.contains("name=\"files[]\""), "{body}");
    assert!(body.contains("d8:announce3:urle"), "{body}");
    assert_eq!(std::fs::read_dir(dir.path().join("torrents")).unwrap().count(), 0, "staged file removed");
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn a_link_that_is_not_a_torrent_is_not_uploaded() {
    let fake = Fake::start(|_| Resp::raw(200, b"<html>login</html>".to_vec()));
    let dir = kroma_module_sdk::testing::temp_dir("ad-html-torrent");
    let alldebrid = engine(&fake, dir.path());

    let page = alldebrid.add(&format!("{}/t.torrent", fake.base)).unwrap_err().to_string();
    let scheme = alldebrid.add("ftp://x/y").unwrap_err().to_string();

    assert!(page.contains("did not return a .torrent"), "{page}");
    assert!(scheme.contains("magnet link"), "{scheme}");
    assert!(fake.calls_to("/v4/magnet/upload/file").is_empty());
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn every_status_code_maps_to_a_state_and_a_progress_in_its_half() {
    let fake = Fake::start(|req| {
        let code: u8 = req.query["id"].parse().unwrap();
        magnet(code, json!({ "id": code, "status": if code >= 5 { "Upload failed" } else { "ok" } }))
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-codes");
    let alldebrid = engine(&fake, dir.path());

    let at = |code: u8| alldebrid.status(&code.to_string()).unwrap().unwrap();

    assert_eq!((at(0).state, at(0).progress), (TorrentState::Queued, 0.0));
    assert_eq!((at(1).state, at(1).progress), (TorrentState::Downloading, 0.225));
    assert_eq!(at(1).down_bps, 99);
    assert_eq!(at(1).peers, 4);
    assert_eq!((at(2).state, at(2).progress), (TorrentState::Downloading, 0.45));
    assert_eq!((at(3).state, at(3).progress), (TorrentState::Downloading, 0.45));
    for code in 5..=15 {
        let failed = at(code);
        assert_eq!(failed.state, TorrentState::Error, "{code}");
        assert!(failed.progress < 0.5, "{code}");
        assert!(failed.error.unwrap().contains("Upload failed"), "{code}");
    }
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn the_status_of_an_unknown_magnet_is_none_and_a_failure_is_an_error() {
    let fake = Fake::start(|req| match req.query["id"].as_str() {
        "1" => Resp::failure("MAGNET_INVALID_ID", "no such magnet"),
        "2" => Resp::data(json!({ "magnets": [] })),
        _ => Resp::failure("AUTH_BAD_APIKEY", "bad"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-none");
    let alldebrid = engine(&fake, dir.path());

    assert!(alldebrid.status("1").unwrap().is_none());
    assert!(alldebrid.status("2").unwrap().is_none());
    assert!(alldebrid.status("3").is_err());
    assert!(alldebrid.status("../etc").is_err());
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn a_single_magnet_answered_as_an_object_is_read_too() {
    let fake = Fake::start(|_| {
        Resp::data(json!({ "magnets": { "id": 7, "filename": "x", "size": 10, "statusCode": 0 } }))
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-object");

    let got = engine(&fake, dir.path()).status("7").unwrap().unwrap();

    assert_eq!(got.name, "x");
}

#[test]
fn pausing_and_resuming_are_refused_and_reannouncing_is_harmless() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-verbs");
    let alldebrid = AllDebrid::new(
        &ClientDef { password: KEY.into(), ..ClientDef::default() },
        dir.path(),
    )
    .unwrap();

    assert!(alldebrid.pause("7").unwrap_err().to_string().contains("cannot pause"));
    assert!(alldebrid.resume("7").is_err());
    assert!(alldebrid.reannounce("7").is_ok());
}

#[test]
#[cfg_attr(target_family = "wasm", ignore = "binds a test server, and a component holds no socket")]
fn listing_files_flattens_and_sanitises_and_a_link_unlocks_afresh() {
    let tree = json!([{ "n": "..", "e": [node("ep1.mkv", 4), node("/etc/passwd", 2)] }]);
    let fake = scripted(tree, vec![("ep1.mkv", b"abcd".to_vec())], true);
    let dir = kroma_module_sdk::testing::temp_dir("ad-list");
    let alldebrid = engine(&fake, dir.path());

    let listed = alldebrid.list_files("7").unwrap();
    let url = alldebrid.direct_url("7", "_/ep1.mkv").unwrap();

    assert_eq!(listed.iter().map(|f| f.rel.as_str()).collect::<Vec<_>>(), ["_/ep1.mkv", "_/_etc_passwd"]);
    assert!(url.starts_with("http://127.0.0.1:") && url.ends_with("/dl/ep1.mkv"), "{url}");
    assert!(alldebrid.direct_url("7", "nope").is_err());
}
