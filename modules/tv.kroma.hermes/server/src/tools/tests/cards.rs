use serde_json::{json, Value};

use crate::tools::validate_card;

fn table() -> Value {
    json!({ "kind": "table", "title": "Classement Ligue 1", "columns": ["#", "Club", "Pts"], "rows": [["1", "PSG", 28], ["2", "Lens", 25]], "note": "après la 11e journée" })
}

fn chart() -> Value {
    json!({ "kind": "chart", "title": "Débit du flux (1 h)", "style": "line", "unit": "kb/s", "points": [{ "label": "20:00", "value": 3800 }, { "label": "20:10", "value": 3650 }] })
}

fn fixture() -> Value {
    json!({ "kind": "match", "home": "PSG", "away": "Liverpool", "competition": "Ligue des champions", "start": 1_793_300_400, "score": "1 - 0", "status": "live", "channel": "Canal+ Foot", "action": { "id": "watch", "label": "Mettre cette chaîne" } })
}

fn with(mut card: Value, key: &str, value: Value) -> Value {
    if value.is_null() {
        card.as_object_mut().unwrap().remove(key);
    } else {
        card[key] = value;
    }
    card
}

fn ok(card: &Value) -> bool {
    validate_card(card).is_some()
}

#[test]
fn the_three_contract_cards_are_valid_and_unchanged() {
    for card in [table(), chart(), fixture()] {
        assert_eq!(validate_card(&card), Some(card.clone()));
    }
}

#[test]
fn fields_outside_the_contract_are_dropped() {
    let dirty = with(with(fixture(), "html", json!("<b>x</b>")), "intrus", json!(1));
    assert_eq!(validate_card(&dirty), Some(fixture()));
}

#[test]
fn the_catalogue_is_closed() {
    for bad in [json!(null), json!("table"), json!([]), json!({}), json!({ "kind": "html" }), with(table(), "kind", json!("Table")), json!({ "kind": [1] }), json!({ "kind": 3 })] {
        assert!(!ok(&bad), "{bad}");
    }
}

#[test]
fn table_bounds() {
    assert!(!ok(&with(with(table(), "columns", json!(vec!["a"; 7])), "rows", json!([vec!["x"; 7]]))));
    assert!(!ok(&with(with(table(), "columns", json!([])), "rows", json!([[]]))));
    assert!(!ok(&with(table(), "rows", json!([]))));
    assert!(!ok(&with(table(), "rows", json!(vec![json!(["1", "PSG", 1]); 13]))));
    assert!(ok(&with(table(), "rows", json!(vec![json!(["1", "PSG", 1]); 12]))));
    assert!(!ok(&with(table(), "rows", json!([["1", "PSG"]]))));
    for cell in [json!(null), json!({ "a": 1 }), json!(true)] {
        assert!(!ok(&with(table(), "rows", json!([["1", "PSG", cell]]))));
    }
    assert!(!ok(&with(table(), "rows", json!([["1", "x".repeat(81), 3]]))));
    assert!(ok(&with(table(), "rows", json!([["1", "x".repeat(80), 3]]))));
    assert!(!ok(&with(table(), "title", json!("t".repeat(61)))));
    assert!(ok(&with(table(), "title", json!("t".repeat(60)))));
    assert!(!ok(&with(table(), "note", json!("n".repeat(81)))));
    assert!(!ok(&with(table(), "columns", json!(["#", 5, "Pts"]))));
}

#[test]
fn a_provisional_table_carries_a_checked_live_block() {
    let live = json!({ "rows": [{ "live": true, "outcome": "win", "move": 1 }, { "live": false, "outcome": null, "move": 0 }], "points_col": 2, "match_col": 1 });
    assert_eq!(validate_card(&with(table(), "live", live.clone())).unwrap()["live"], live);
    assert!(!ok(&with(table(), "live", json!({ "rows": [{ "live": true }] }))));
    assert!(!ok(&with(table(), "live", json!({ "rows": [{ "outcome": "tie" }, {}] }))));
    assert!(!ok(&with(table(), "live", json!({ "rows": [{ "move": 21 }, {}] }))));
    assert!(!ok(&with(table(), "live", json!({ "rows": [{}, {}], "points_col": 3 }))));
}

#[test]
fn chart_bounds() {
    let point = json!({ "label": "a", "value": 1 });
    assert!(!ok(&with(chart(), "points", json!([point]))));
    assert!(!ok(&with(chart(), "points", json!(vec![point.clone(); 25]))));
    assert!(ok(&with(chart(), "points", json!(vec![point.clone(); 24]))));
    assert!(!ok(&with(chart(), "style", json!("pie"))));
    assert!(ok(&with(chart(), "style", json!("bar"))));
    assert!(!ok(&with(chart(), "unit", json!("u".repeat(13)))));
    assert!(ok(&with(chart(), "unit", json!("u".repeat(12)))));
    assert!(ok(&with(chart(), "unit", json!(null))));
    assert!(!ok(&with(chart(), "points", json!([point.clone(), { "label": "b", "value": "3" }]))));
    assert!(!ok(&with(chart(), "points", json!([point.clone(), { "label": "b" }]))));
    assert!(!ok(&with(chart(), "points", json!([point, "b"]))));
}

#[test]
fn match_bounds() {
    assert!(!ok(&with(fixture(), "home", json!(null))));
    assert!(!ok(&with(fixture(), "away", json!(""))));
    assert!(!ok(&with(fixture(), "status", json!("paused"))));
    assert!(!ok(&with(fixture(), "start", json!("demain"))));
    assert_eq!(validate_card(&json!({ "kind": "match", "home": "A", "away": "B" })), Some(json!({ "kind": "match", "home": "A", "away": "B" })));
    assert!(!ok(&with(fixture(), "channel", json!(null))));
    assert!(!ok(&with(fixture(), "action", json!({ "id": "delete", "label": "x" }))));
    assert!(!ok(&with(fixture(), "action", json!({ "id": "watch" }))));
    assert!(!ok(&with(fixture(), "action", json!("watch"))));
    assert!(ok(&with(fixture(), "action", json!(null))));
}

#[test]
fn the_start_is_in_seconds_between_2000_and_2100() {
    for good in [946_684_800_i64, 1_793_300_400, 4_102_444_800] {
        assert!(ok(&with(fixture(), "start", json!(good))), "{good}");
    }
    for bad in [1_793_300_400_000_i64, 4_102_444_801, 946_684_799, 0, -1] {
        assert!(!ok(&with(fixture(), "start", json!(bad))), "{bad}");
    }
}

#[test]
fn strings_are_kept_raw_and_counted_in_code_points() {
    let trap = "<img src=x onerror=alert(1)>";
    assert_eq!(validate_card(&with(table(), "title", json!(trap))).unwrap()["title"], trap);
    assert!(ok(&with(fixture(), "home", json!("😀".repeat(80)))));
    assert!(!ok(&with(fixture(), "home", json!("😀".repeat(81)))));
}
