use serde_json::json;

use crate::fake_alldebrid::{Fake, Resp};
use crate::it_engine::{engine, magnet, KEY};
use crate::{AllDebrid, ClientDef, TorrentState};

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn every_status_code_maps_to_a_state_and_a_progress_in_its_half() {
    let fake = Fake::start(|req| {
        let code: u8 = req.form()["id"].parse().unwrap();
        magnet(
            code,
            json!({ "id": code, "status": if code >= 5 { "Upload failed" } else { "ok" } }),
        )
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-codes");
    let alldebrid = engine(&fake, dir.path());

    let at = |code: u8| alldebrid.status(&code.to_string()).unwrap().unwrap();

    assert_eq!((at(0).state, at(0).progress), (TorrentState::Queued, 0.0));
    assert_eq!(
        (at(1).state, at(1).progress),
        (TorrentState::Downloading, 0.225)
    );
    assert_eq!(at(1).down_bps, 99);
    assert_eq!(at(1).peers, 4);
    assert_eq!(
        (at(2).state, at(2).progress),
        (TorrentState::Downloading, 0.45)
    );
    assert_eq!(
        (at(3).state, at(3).progress),
        (TorrentState::Downloading, 0.45)
    );
    for code in 5..=15 {
        let failed = at(code);
        assert_eq!(failed.state, TorrentState::Error, "{code}");
        assert!(failed.progress < 0.5, "{code}");
        assert!(failed.error.unwrap().contains("Upload failed"), "{code}");
    }
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn the_status_of_an_unknown_magnet_is_none_and_a_failure_is_an_error() {
    let fake = Fake::start(|req| match req.form()["id"].as_str() {
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
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
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
        &ClientDef {
            password: KEY.into(),
            ..ClientDef::default()
        },
        dir.path(),
    )
    .unwrap();

    assert!(alldebrid
        .pause("7")
        .unwrap_err()
        .to_string()
        .contains("cannot pause"));
    assert!(alldebrid.resume("7").is_err());
    assert!(alldebrid.reannounce("7").is_ok());
}
