use kroma_module_sdk::domain::RequestKind;
use serde_json::json;

use super::*;

#[test]
fn a_show_is_asked_for_one_episode_at_a_time() {
    let ask = Ask {
        kind: "show".into(),
        tmdb_id: 42,
        ..Ask::default()
    };

    assert!(asked_unit(RequestKind::Show, &ask).is_err());
    assert_eq!(asked_unit(RequestKind::Movie, &ask).unwrap(), None);
}

#[test]
fn the_reply_crosses_as_a_status_tag_with_snake_case_fields() {
    let imported = serde_json::to_value(Reply::Imported(Imported {
        request_id: "r".into(),
        download_id: "d".into(),
        release: Release {
            key: "k".into(),
            title: "T".into(),
            claimed_height: Some(1080),
        },
        paths: vec!["/a".into()],
        primary_path: "/a".into(),
        episodes: 2,
        multi_part: false,
    }))
    .unwrap();

    assert_eq!(
        imported,
        json!({
            "status": "imported", "request_id": "r", "download_id": "d",
            "release": { "key": "k", "title": "T", "claimed_height": 1080 },
            "paths": ["/a"], "primary_path": "/a", "episodes": 2, "multi_part": false
        })
    );
    assert_eq!(
        serde_json::to_value(Reply::Requested {
            request_id: "r".into()
        })
        .unwrap(),
        json!({ "status": "requested", "request_id": "r" })
    );
    assert_eq!(
        serde_json::to_value(Reply::Present { request_id: None }).unwrap(),
        json!({ "status": "present", "request_id": null })
    );
    assert_eq!(
        serde_json::to_value(Reply::Unavailable {
            reason: Why::NoIndexer,
            request_id: None
        })
        .unwrap(),
        json!({ "status": "unavailable", "reason": "noIndexer", "request_id": null })
    );
}

#[test]
fn an_ask_reads_from_the_core_s_json_and_tolerates_missing_fields() {
    let ask: Ask =
        serde_json::from_value(json!({ "kind": "movie", "tmdb_id": 1, "user_id": "u" })).unwrap();

    assert!(ask.exclude.is_empty());
    assert_eq!(ask.budget_ms, None);
}
