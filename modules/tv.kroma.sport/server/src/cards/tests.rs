use serde_json::{json, Value};

use super::validate;

fn table() -> Value {
    json!({"kind": "table", "title": "Classement Ligue 1", "columns": ["#", "Club", "Pts"], "rows": [["1", "PSG", 28], ["2", "Lens", 25]], "note": "après la 11e journée"})
}

fn chart() -> Value {
    json!({"kind": "chart", "title": "Débit du flux (1 h)", "style": "line", "unit": "kb/s", "points": [{"label": "20:00", "value": 3800}, {"label": "20:10", "value": 3650}]})
}

fn match_card() -> Value {
    json!({"kind": "match", "home": "PSG", "away": "Liverpool", "competition": "Ligue des champions", "start": 1_793_300_400, "score": "1 - 0", "status": "live", "channel": "Canal+ Foot", "action": {"id": "watch", "label": "Mettre cette chaîne"}})
}

fn with(mut card: Value, key: &str, value: Value) -> Value {
    card[key] = value;
    card
}

fn without(mut card: Value, key: &str) -> Value {
    card.as_object_mut().unwrap().remove(key);
    card
}

#[test]
fn a_card_of_the_contract_comes_back_unaltered_and_foreign_fields_are_dropped() {
    for card in [table(), chart(), match_card()] {
        assert_eq!(validate(&card), Some(card.clone()));
    }

    assert_eq!(
        validate(&with(
            with(match_card(), "html", json!("<b>")),
            "intrus",
            json!(1)
        )),
        Some(match_card())
    );
}

#[test]
fn the_catalogue_is_closed() {
    for invalid in [
        json!(null),
        json!("table"),
        json!([]),
        json!({}),
        json!({"kind": "html"}),
        with(table(), "kind", json!("Table")),
        json!({"kind": "__proto__"}),
        json!({"kind": "constructor"}),
    ] {
        assert_eq!(validate(&invalid), None, "{invalid}");
    }
}

#[test]
fn the_live_metadata_of_a_provisional_table_is_kept_when_valid_and_refused_otherwise() {
    let live = json!({"rows": [{"live": true, "outcome": "win", "move": 1}, {"live": false, "outcome": null, "move": 0}], "points_col": 2});

    assert_eq!(
        validate(&with(table(), "live", live.clone())),
        Some(with(table(), "live", live))
    );
    for bad in [
        json!({"rows": [{"live": true}]}),
        json!({"rows": [{"outcome": "boom"}, {}]}),
        json!({"rows": [{}, {}], "points_col": 9}),
        json!({"rows": [{"move": 99}, {}]}),
    ] {
        assert_eq!(validate(&with(table(), "live", bad.clone())), None, "{bad}");
    }
}

#[test]
fn a_table_keeps_to_its_bounds() {
    let wide = with(
        with(table(), "columns", json!(vec!["a"; 7])),
        "rows",
        json!([vec!["x"; 7]]),
    );
    let rows = |n: usize| with(table(), "rows", json!(vec![json!(["1", "PSG", 1]); n]));

    assert_eq!(validate(&wide), None);
    assert_eq!(validate(&rows(0)), None);
    assert_eq!(validate(&rows(13)), None);
    assert!(validate(&rows(12)).is_some());
    for cells in [
        json!([["1", "PSG"]]),
        json!([["1", "PSG", null]]),
        json!([["1", "PSG", true]]),
        json!([["1", "PSG", {"a": 1}]]),
        json!([["1", "x".repeat(81), 3]]),
    ] {
        assert_eq!(
            validate(&with(table(), "rows", cells.clone())),
            None,
            "{cells}"
        );
    }
    assert!(validate(&with(table(), "rows", json!([["1", "x".repeat(80), 3]]))).is_some());
    assert_eq!(
        (
            validate(&with(table(), "title", json!("t".repeat(61)))),
            validate(&with(table(), "title", json!("t".repeat(60)))).is_some()
        ),
        (None, true)
    );
}

#[test]
fn a_chart_needs_a_style_two_to_twenty_four_finite_points_and_a_short_unit() {
    let points = |n: usize| {
        with(
            chart(),
            "points",
            json!((0..n)
                .map(|i| json!({"label": format!("{i}"), "value": i}))
                .collect::<Vec<_>>()),
        )
    };

    assert_eq!((validate(&points(1)), validate(&points(25))), (None, None));
    assert!(validate(&points(24)).is_some());
    assert_eq!(validate(&with(chart(), "style", json!("pie"))), None);
    assert_eq!(
        validate(&with(
            chart(),
            "points",
            json!([{"label": "a", "value": "x"}, {"label": "b", "value": 1}])
        )),
        None
    );
    assert_eq!(
        validate(&with(chart(), "unit", json!("u".repeat(13)))),
        None
    );
}

#[test]
fn a_match_card_needs_both_teams_a_start_in_seconds_a_known_status_and_a_channel_for_its_action() {
    assert_eq!(validate(&without(match_card(), "home")), None);
    assert_eq!(
        validate(&with(match_card(), "start", json!(1_793_300_400_000_i64))),
        None
    );
    assert_eq!(
        validate(&with(match_card(), "start", json!(946_684_799))),
        None
    );
    assert!(validate(&with(match_card(), "start", json!(946_684_800))).is_some());
    assert_eq!(
        validate(&with(match_card(), "status", json!("paused"))),
        None
    );
    assert_eq!(validate(&without(match_card(), "channel")), None);
    assert_eq!(
        validate(&with(
            match_card(),
            "action",
            json!({"id": "delete", "label": "x"})
        )),
        None
    );
    assert!(validate(&without(without(match_card(), "channel"), "action")).is_some());
}
