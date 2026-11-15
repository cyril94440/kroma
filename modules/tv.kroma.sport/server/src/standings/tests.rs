use serde_json::{json, Value};

use super::{competition_code, from_espn, provisional, LiveGame, StandingRow};
use crate::cards::validate;

fn row(club: &str, pts: i64, diff: i64) -> StandingRow {
    StandingRow {
        club: json!(club),
        pts: json!(pts),
        diff: json!(diff),
    }
}

fn game(home: &str, away: &str, score: &str, minute: &str) -> LiveGame {
    LiveGame {
        home: json!(home),
        away: json!(away),
        score: json!(score),
        minute: json!(minute),
    }
}

fn base() -> Vec<StandingRow> {
    vec![
        row("Paris Saint-Germain", 17, 7),
        row("Stade Rennais", 16, 4),
        row("LOSC Lille", 15, 5),
        row("AS Monaco", 15, 4),
        row("Le Havre AC", 2, -14),
    ]
}

fn line(result: &Value, club: &str) -> Value {
    result["lignes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["club"] == club)
        .unwrap()
        .clone()
}

#[test]
fn live_games_add_provisional_points_and_sort_the_table() {
    let games = [
        game("Le Havre", "PSG", "0-2", "67'"),
        game("Lille", "Monaco", "1 - 1", ""),
    ];

    let result = provisional(&base(), &games, "Ligue 1", 10).unwrap().json;

    assert_eq!(
        (
            line(&result, "Paris Saint-Germain")["pts"].clone(),
            line(&result, "Paris Saint-Germain")["diff"].clone()
        ),
        (json!(20), json!(9))
    );
    assert_eq!(
        (
            line(&result, "Le Havre AC")["pts"].clone(),
            line(&result, "Le Havre AC")["diff"].clone()
        ),
        (json!(2), json!(-16))
    );
    assert_eq!(
        (
            line(&result, "LOSC Lille")["pts"].clone(),
            line(&result, "AS Monaco")["pts"].clone()
        ),
        (json!(16), json!(16))
    );
    let order: Vec<&str> = result["lignes"]
        .as_array()
        .unwrap()
        .iter()
        .take(3)
        .map(|l| l["club"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        ["Paris Saint-Germain", "LOSC Lille", "Stade Rennais"]
    );
    assert_eq!(
        (
            result["matchs_comptes"].clone(),
            result.get("non_apparies").is_none()
        ),
        (json!(2), true)
    );
    assert_eq!(
        (
            line(&result, "Paris Saint-Germain")["match"].clone(),
            line(&result, "Le Havre AC")["match"].clone()
        ),
        (json!("2-0 67'"), json!("0-2 67'"))
    );
}

#[test]
fn the_card_is_valid_for_a_chat_message() {
    let result = provisional(
        &base(),
        &[game("Le Havre", "PSG", "0-2", "67'")],
        "Ligue 1",
        10,
    )
    .unwrap()
    .json;

    let card = validate(&result["carte"]).unwrap();

    assert_eq!(
        card["rows"][0].as_array().unwrap()[..3],
        [json!("1"), json!("Paris Saint-Germain"), json!("20")]
    );
    assert!(card["title"].as_str().unwrap().contains("provisoire"));
}

#[test]
fn an_unknown_club_is_reported_and_an_unreadable_score_skipped() {
    let games = [
        game("Brest", "PSG", "1-0", ""),
        game("Lille", "Monaco", "mi-temps", ""),
    ];

    let result = provisional(&base(), &games, "", 10).unwrap().json;

    assert_eq!(result["non_apparies"], json!(["Brest"]));
    assert!(!result["scores_illisibles"].as_array().unwrap().is_empty());
    assert_eq!(
        (
            result["matchs_comptes"].clone(),
            line(&result, "Paris Saint-Germain")["pts"].clone()
        ),
        (json!(1), json!(17))
    );
}

#[test]
fn no_live_game_means_the_official_table_and_an_empty_start_is_an_error() {
    let result = provisional(&base(), &[], "", 10).unwrap().json;

    assert_eq!(result["matchs_comptes"], 0);
    assert!(result["carte"]["note"]
        .as_str()
        .unwrap()
        .contains("officiel"));
    assert!(provisional(&[], &[game("a", "b", "1-0", "")], "", 10).is_err());
}

#[test]
fn only_games_in_play_are_read_from_espn() {
    let team = |name: &str, pts: i64, diff: i64| json!({"team": {"displayName": name}, "stats": [{"name": "points", "value": pts}, {"name": "pointDifferential", "value": diff}]});
    let table = json!({"children": [{"standings": {"entries": [team("Paris Saint-Germain", 17, 7), team("Stade Rennais", 16, 4), team("Nice", 12, 1), team("Le Havre AC", 2, -14)]}}]});
    let event = |state: &str, home: &str, hs: &str, away: &str, aws: &str, clock: &str| json!({"competitions": [{"status": {"displayClock": clock, "type": {"state": state}}, "competitors": [{"homeAway": "home", "score": hs, "team": {"displayName": home}}, {"homeAway": "away", "score": aws, "team": {"displayName": away}}]}]});
    let scores = json!({"events": [event("in", "Le Havre AC", "1", "Paris Saint-Germain", "2", "44'"), event("in", "Nice", "2", "Stade Rennais", "1", "45'"), event("pre", "Lille", "0", "Lens", "0", "0'")]});

    let (rows, games) = from_espn(&table, &scores);
    let result = provisional(&rows, &games, "Ligue 1", 10).unwrap().json;

    assert_eq!((rows.len(), games.len()), (4, 2));
    assert_eq!(
        (
            line(&result, "Paris Saint-Germain")["pts"].clone(),
            line(&result, "Stade Rennais")["pts"].clone(),
            line(&result, "Nice")["pts"].clone()
        ),
        (json!(20), json!(16), json!(15))
    );
    assert!(result.get("non_apparies").is_none());
    assert_eq!(
        (
            competition_code("Ligue 1"),
            competition_code("la Liga"),
            competition_code("Ligue des champions")
        ),
        (Some("fra.1"), Some("esp.1"), Some("uefa.champions"))
    );
}

#[test]
fn the_live_card_carries_outcome_and_movement_against_the_official_table() {
    let result = provisional(
        &base(),
        &[game("Lille", "PSG", "2-0", "80'")],
        "Ligue 1",
        10,
    )
    .unwrap()
    .json;

    let card = validate(&result["carte"]).unwrap();
    let clubs: Vec<&str> = card["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r[1].as_str().unwrap())
        .collect();
    let at =
        |club: &str| card["live"]["rows"][clubs.iter().position(|c| *c == club).unwrap()].clone();
    let bad = json!({"kind": "table", "columns": ["a"], "rows": [["x"]], "live": {"rows": [{"live": true, "outcome": "boom"}]}});

    assert_eq!(
        at("LOSC Lille"),
        json!({"live": true, "outcome": "win", "move": 2})
    );
    assert_eq!(
        (
            at("Paris Saint-Germain")["outcome"].clone(),
            at("Paris Saint-Germain")["move"].clone()
        ),
        (json!("loss"), json!(-1))
    );
    assert_eq!(
        at("Stade Rennais"),
        json!({"live": false, "outcome": null, "move": -1})
    );
    assert!(validate(&bad).is_none());
}
