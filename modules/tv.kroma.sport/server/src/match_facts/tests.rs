use serde_json::{json, Value};

use super::{approximate_minute, facts, list_line, search};
use crate::detect::{flat_matches, FlatMatch};
use crate::iso::parse_secs;

fn finished() -> Value {
    serde_json::from_str(include_str!("../../fixtures/espn_summary_termine.json")).unwrap()
}

fn in_play() -> Value {
    serde_json::from_str(include_str!("../../fixtures/espn_summary_en_cours.json")).unwrap()
}

fn at(iso: &str) -> f64 {
    parse_secs(iso).unwrap()
}

fn board(games: &[(&str, &str, &str, &str, &str, &str)]) -> Vec<FlatMatch> {
    let team = |side: &str, name: &str, score: &str| json!({"homeAway": side, "score": score, "team": {"displayName": name, "shortDisplayName": name, "name": name, "location": name, "abbreviation": name[..3].to_uppercase()}});
    let events: Vec<Value> = games
        .iter()
        .map(|(id, home, away, state, hs, aws)| {
            json!({"id": id, "date": "2026-10-30T20:05Z", "competitions": [{"date": "2026-10-30T20:05Z", "status": {"displayClock": "80'", "period": 2, "type": {"state": state}}, "competitors": [team("home", home, hs), team("away", away, aws)]}]})
        })
        .collect();
    flat_matches(&json!({"events": events}), "fra.1")
}

#[test]
fn a_finished_match_gives_scorers_cards_and_the_score() {
    let r = facts(&finished(), at("2026-10-30T22:00:00Z"), 60.0);

    assert_eq!(
        (
            r["match"].as_str(),
            r["statut"].as_str(),
            r["score"].as_str()
        ),
        (Some("Nice – Stade Rennais"), Some("termine"), Some("5-2"))
    );
    assert_eq!(r["buts"].as_array().unwrap().len(), 7);
    assert_eq!(
        r["buts"][0],
        json!({"minute": "12'", "equipe": "Nice", "buteur": "Elye Wahi", "type": "penalty"})
    );
    assert_eq!(
        (
            r["buts"][1]["buteur"].clone(),
            r["buts"][1]["equipe"].clone(),
            r["buts"][1]["passeur"].clone()
        ),
        (
            json!("Sebastian Szymanski"),
            json!("Stade Rennais"),
            json!("Mahamadou Nagida")
        )
    );
    assert!(r["cartons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["joueur"] == "Anthony Rouault"
            && c["carton"] == "jaune"
            && c["minute"] == "61'"));
    assert!(r.get("note").is_none() && r["source"] == "espn");
}

#[test]
fn a_goal_newer_than_the_picture_is_hidden_until_the_delay_has_passed() {
    let goal = at("2026-10-30T21:47:48Z");

    let hidden = facts(&in_play(), goal + 30.0, 60.0);
    let shown = facts(&in_play(), goal + 90.0, 60.0);

    assert_eq!(
        (
            hidden["statut"].clone(),
            hidden["score"].clone(),
            hidden["buts"].clone(),
            hidden["minute"].clone()
        ),
        (json!("en_cours"), json!("0-0"), json!([]), json!("90'+"))
    );
    assert!(hidden["note"].as_str().unwrap().contains("décalées"));
    assert_eq!(
        (shown["score"].clone(), shown["buts"][0]["equipe"].clone()),
        (json!("1-0"), json!("Lille"))
    );
    assert!(shown["buts"][0]["buteur"].is_string());
    assert_eq!(
        shown["cartons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["joueur"].clone())
            .collect::<Vec<_>>(),
        [json!("Saud Abdulhamid")]
    );
}

#[test]
fn the_final_whistle_is_not_exposed_before_the_picture_reaches_it() {
    let summary = finished();
    let last = summary["keyEvents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|k| parse_secs(k["wallclock"].as_str()?))
        .fold(0.0, f64::max);

    let soon = facts(&summary, last + 20.0, 60.0);
    let later = facts(&summary, last + 120.0, 60.0);

    assert!(soon["statut"] == "en_cours" && soon.get("note").is_some());
    assert_eq!(
        (later["statut"].clone(), later["score"].clone()),
        (json!("termine"), json!("5-2"))
    );
}

#[test]
fn the_minute_is_shifted_by_the_delay_and_an_untimed_live_match_exposes_nothing() {
    let mut summary = in_play();
    for key in summary["keyEvents"].as_array_mut().unwrap() {
        key.as_object_mut().unwrap().remove("wallclock");
    }

    let r = facts(&summary, at("2026-10-30T23:00:00Z"), 60.0);

    assert_eq!(
        (
            approximate_minute("67'", 60.0),
            approximate_minute("1'", 60.0),
            approximate_minute("45'+3'", 60.0)
        ),
        ("~66'".to_string(), "~1'".to_string(), "45'+".to_string())
    );
    assert_eq!(
        (r["buts"].clone(), r["score"].clone()),
        (json!([]), json!("0-0"))
    );
}

#[test]
fn an_unreadable_answer_and_an_upcoming_match_are_handled() {
    let mut summary = in_play();
    summary["header"]["competitions"][0]["status"]["type"]["state"] = "pre".into();

    let upcoming = facts(&summary, 0.0, 60.0);

    assert!(facts(&json!({"header": {}}), 0.0, 60.0)
        .get("error")
        .is_some());
    assert_eq!(upcoming["statut"], "a_venir");
    assert!(
        upcoming.get("score").is_none()
            && upcoming.get("buts").is_none()
            && upcoming["debut"].is_string()
    );
}

#[test]
fn teams_are_searched_without_inventing_or_confusing_a_match() {
    let matches = board(&[
        ("1", "Lille", "Lens", "in", "1", "0"),
        ("2", "Paris Saint-Germain", "Le Havre AC", "post", "1", "5"),
    ]);
    let ids = |query: Value| {
        search(&matches, &query)
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(ids(json!("Lens - Lille")), ["espn-1"]);
    assert_eq!(ids(json!("Lens")), ["espn-1"]);
    assert_eq!(ids(json!(["PSG", "Le Havre"])), ["espn-2"]);
    assert!(ids(json!("Lens - Paris FC")).is_empty() && ids(json!("")).is_empty());
    let (live, done) = (list_line(&matches[0]), list_line(&matches[1]));
    assert!(
        live["score"].as_str().unwrap().contains("décalé")
            && !live["score"].as_str().unwrap().contains("1-0")
    );
    assert_eq!(
        (done["score"].clone(), done["statut"].clone()),
        (json!("1-5"), json!("termine"))
    );
}
