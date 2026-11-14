use super::*;
use serde_json::json;

#[test]
fn an_imported_reply_reads_the_modules_json() {
    let reply: Reply = serde_json::from_value(json!({
        "status": "imported", "request_id": "r", "download_id": "d",
        "release": { "key": "k", "title": "T", "claimed_height": 1080 },
        "paths": ["/a", "/b"], "primary_path": "/a", "episodes": 2, "multi_part": true
    }))
    .unwrap();

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.claimed_height, Some(1080));
    assert_eq!(imported.paths.len(), 2);
    assert_eq!((imported.episodes, imported.multi_part), (2, true));
}

#[test]
fn an_imported_reply_without_the_newer_fields_still_reads() {
    let reply: Reply = serde_json::from_value(json!({
        "status": "imported", "request_id": "r", "download_id": "d",
        "release": { "key": "k", "title": "T" }, "paths": [], "primary_path": "/a"
    }))
    .unwrap();

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!((imported.episodes, imported.multi_part), (1, false));
    assert_eq!(imported.release.claimed_height, None);
}

#[test]
fn a_present_reply_reads_with_or_without_a_request() {
    let held: Reply = serde_json::from_value(json!({ "status": "present", "request_id": null })).unwrap();
    let named: Reply = serde_json::from_value(json!({ "status": "present", "request_id": "r" })).unwrap();

    assert!(matches!(held, Reply::Present { request_id: None }));
    assert!(matches!(named, Reply::Present { request_id: Some(_) }));
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
    let rejected = serde_json::to_value(Rejection {
        download_id: "d",
        reason: "r",
    })
    .unwrap();
    let confirmed = serde_json::to_value(Confirmation { download_id: "d" }).unwrap();
    let flagged = serde_json::to_value(Flag {
        kind: "movie",
        tmdb_id: 1,
        release_title: "T",
        reason: "r",
    })
    .unwrap();

    assert_eq!(
        asked,
        json!({ "kind": "show", "tmdb_id": 1, "season": 1, "episode": 2, "user_id": "u", "exclude": ["g|i"], "budget_ms": 20000 })
    );
    assert_eq!(rejected, json!({ "download_id": "d", "reason": "r" }));
    assert_eq!(confirmed, json!({ "download_id": "d" }));
    assert_eq!(
        flagged,
        json!({ "kind": "movie", "tmdb_id": 1, "release_title": "T", "reason": "r" })
    );
}
