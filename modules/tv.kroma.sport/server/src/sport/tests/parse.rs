use serde_json::json;

use super::support::{fixture, NOW};
use crate::sport::{days_to_read, other, soccer, Sport, State};

#[test]
fn reads_the_real_espn_formats() {
    let soccer = soccer::soccer(&fixture("foot"), "fra.1", "Ligue 1", 100);
    let tennis = other::tennis(&fixture("tennis"), "atp", 1_793_800_000.0);
    let f1 = other::f1(&fixture("f1"), 1_793_800_000.0);
    let golf = other::golf(&fixture("golf"));

    assert_eq!(
        (soccer.len(), soccer[0].sport, soccer[0].lines.len()),
        (1, Sport::Soccer, 2)
    );
    assert!(tennis
        .iter()
        .all(|e| !e.competition.to_lowercase().contains("qualif")));
    assert!(f1
        .iter()
        .any(|e| e.competition == "Course" || e.competition.starts_with("Essais")));
    assert!(!f1[0].lines[0].name.contains("MSC"));
    assert_eq!((golf.len(), golf[0].sport), (1, Sport::Golf));
}

#[test]
fn reads_one_day_at_a_time_and_includes_yesterday_at_night() {
    let (near, far) = days_to_read(NOW);

    assert!(near
        .iter()
        .chain(&far)
        .all(|d| d.len() == 8 && d.chars().all(|c| c.is_ascii_digit())));
    assert_eq!(far.len(), 2);
    assert!((1..=2).contains(&days_to_read(1_793_840_400.0).0.len()));
}

fn tennis_match(id: i64, name: &str, state: &str) -> serde_json::Value {
    json!({
        "id": id.to_string(), "date": "2026-11-05T20:00Z", "status": {"type": {"state": state}},
        "round": {"displayName": "Round 3"},
        "competitors": [{"athlete": {"displayName": name}}, {"athlete": {"displayName": "Y"}}]
    })
}

fn masters(matches: Vec<serde_json::Value>) -> serde_json::Value {
    json!({"events": [{"name": "Rolex Paris Masters", "groupings": [{"grouping": {"displayName": "Men's Singles"}, "competitions": matches}]}]})
}

#[test]
fn tennis_drops_unnamed_players_and_caps_upcoming_matches() {
    let mut matches = vec![tennis_match(1, "TBD", "pre")];
    matches.extend((2..9).map(|i| tennis_match(i, &format!("J{i}"), "pre")));
    matches.push(tennis_match(9, "L", "in"));

    let events = other::tennis(&masters(matches), "atp", NOW);

    assert!(events
        .iter()
        .all(|e| e.lines.iter().all(|l| l.name != "TBD")));
    assert_eq!(events.iter().filter(|e| e.state == State::Pre).count(), 4);
    assert_eq!(events.iter().filter(|e| e.state == State::In).count(), 1);
}

#[test]
fn women_circuit_keeps_only_the_big_tournaments() {
    let payload = json!({"events": [{"name": "Angola Ladies Open", "groupings": masters(vec![tennis_match(1, "A", "pre")])["events"][0]["groupings"]}]});

    assert!(other::tennis(&payload, "wta", NOW).is_empty());
}

#[test]
fn a_postponed_match_is_dropped() {
    let mut payload = fixture("foot");
    payload["events"][0]["competitions"][0]["status"] =
        json!({"type": {"state": "post", "completed": false, "name": "STATUS_POSTPONED"}});

    assert!(soccer::soccer(&payload, "fra.1", "Ligue 1", 100).is_empty());
}
