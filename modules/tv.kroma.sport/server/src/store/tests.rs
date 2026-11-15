use serde_json::json;

use super::{test_store, NewClip};

fn clip(id: &str, to_ms: i64) -> NewClip {
    NewClip {
        id: id.into(),
        match_id: "espn-1".into(),
        ev_id: "a1".into(),
        dom: "Lens".into(),
        ext: "Lille".into(),
        competition: "Ligue 1".into(),
        minute: "20'".into(),
        equipe: "dom".into(),
        buteur: "Kebbal".into(),
        score: "1-0".into(),
        debut_ms: to_ms - 110_000,
        fin_ms: to_ms,
        duree_ms: 110_000,
        path: format!("clips/{id}.mp4"),
        ts: to_ms,
    }
}

#[test]
fn clips_are_listed_newest_first_and_filtered_by_match() {
    let (store, _guard) = test_store("sport-clips");
    store.clip_insert(&clip("one", 1_000_000)).unwrap();
    store
        .clip_insert(&NewClip {
            match_id: "espn-2".into(),
            ..clip("two", 2_000_000)
        })
        .unwrap();

    let all = store.clips(None, 10).unwrap();
    let only = store.clips(Some("espn-1"), 10).unwrap();

    assert_eq!(
        all.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["two", "one"]
    );
    assert_eq!(
        only.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["one"]
    );
}

#[test]
fn retention_finds_the_old_and_the_overflow() {
    let (store, _guard) = test_store("sport-retention");
    (1..=5).for_each(|i| {
        store
            .clip_insert(&clip(&format!("c{i}"), i * 1_000))
            .unwrap()
    });

    let old = store.clips_older_than(3_000).unwrap();
    let over = store.clips_over(3).unwrap();
    store.clip_delete("c1").unwrap();

    assert_eq!(
        old.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["c1", "c2"]
    );
    assert_eq!(
        over.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["c2", "c1"]
    );
    assert!(store.clip("c1").unwrap().is_none());
}

#[test]
fn attempts_count_up_per_goal() {
    let (store, _guard) = test_store("sport-attempts");

    store.attempt_add("g", 1).unwrap();
    store.attempt_add("g", 2).unwrap();

    assert_eq!(store.attempts().unwrap()["g"], 2);
}

#[test]
fn the_black_box_keeps_lines_by_genre_and_prunes_by_age() {
    let (store, _guard) = test_store("sport-trace");
    store.trace(1_000, "espn", "score", &json!({"a": 1}));
    store.trace(2_000, "clips", "goal", &json!({"b": 2}));

    let espn = store.trace_rows(Some("espn"), None, 10).unwrap();
    let pruned = store.trace_prune(1_500).unwrap();

    assert_eq!((espn.len(), espn[0].data.clone()), (1, json!({"a": 1})));
    assert_eq!(pruned, 1);
    assert_eq!(store.trace_rows(None, None, 10).unwrap().len(), 1);
}

#[test]
fn state_blobs_round_trip_through_the_key_value_table() {
    let (store, _guard) = test_store("sport-kv");

    store.kv_put("memo", &json!({"image": {}}));

    assert_eq!(
        store.kv_json::<serde_json::Value>("memo"),
        Some(json!({"image": {}}))
    );
    assert_eq!(store.kv_json::<serde_json::Value>("absent"), None);
}
