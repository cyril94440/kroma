use super::*;
use serde_json::json;

#[test]
fn an_imported_reply_reads_the_modules_json() {
    let reply: Reply = serde_json::from_value(json!({
        "status": "imported", "request_id": "r", "download_id": "d",
        "release": { "key": "k", "title": "T", "claimed_height": 1080 },
        "paths": ["/a", "/b"], "primary_path": "/a"
    }))
    .unwrap();

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.claimed_height, Some(1080));
    assert_eq!(imported.paths.len(), 2);
}

#[test]
fn a_requested_and_an_unavailable_reply_read_too() {
    let requested: Reply =
        serde_json::from_value(json!({ "status": "requested", "request_id": "r" })).unwrap();
    let unavailable: Reply = serde_json::from_value(
        json!({ "status": "unavailable", "reason": "noRelease", "request_id": null }),
    )
    .unwrap();

    assert!(matches!(requested, Reply::Requested { .. }));
    assert!(matches!(
        unavailable,
        Reply::Unavailable {
            reason: Why::NoRelease,
            request_id: None
        }
    ));
}

#[test]
fn a_reason_a_newer_module_invents_reads_as_other() {
    let reply: Reply =
        serde_json::from_value(json!({ "status": "unavailable", "reason": "tomorrow" })).unwrap();

    assert!(matches!(
        reply,
        Reply::Unavailable {
            reason: Why::Other,
            ..
        }
    ));
}

#[test]
fn the_ask_and_the_rejection_carry_the_fields_the_module_reads() {
    let exclude = vec!["g|i".to_string()];
    let asked = serde_json::to_value(Ask {
        kind: "show",
        tmdb_id: 1,
        season: Some(1),
        episode: Some(2),
        user_id: "u",
        exclude: &exclude,
        budget_ms: 20_000,
    })
    .unwrap();
    let paths = vec!["/a".to_string()];
    let rejected = serde_json::to_value(Rejection {
        download_id: "d",
        release_title: "T",
        kind: "movie",
        tmdb_id: 1,
        reason: "r",
        paths: &paths,
        request_id: "r1",
        season: None,
    })
    .unwrap();

    assert_eq!(
        asked,
        json!({ "kind": "show", "tmdb_id": 1, "season": 1, "episode": 2, "user_id": "u", "exclude": ["g|i"], "budget_ms": 20000 })
    );
    assert_eq!(
        rejected,
        json!({ "download_id": "d", "release_title": "T", "kind": "movie", "tmdb_id": 1, "reason": "r",
                "paths": ["/a"], "request_id": "r1", "season": null })
    );
}
