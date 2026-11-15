use serde_json::{json, Value};

use super::support::{call, fail, history, point, reply};
use crate::peers::HistoryPoint;
use crate::testing::Rig;
use crate::tools::validate_card;

fn points(n: i64, kbps: impl Fn(i64) -> f64) -> Vec<HistoryPoint> {
    (0..n).map(|i| point(i, kbps(i))).collect()
}

fn run(rig: &Rig, args: Value) -> Value {
    call(rig, "stream_history", args, "Mamie")
}

#[test]
fn an_hour_of_rate_becomes_a_valid_line_card_and_a_summary_computed_by_the_code() {
    let rig = Rig::new();
    let data = points(360, |i| if (200..203).contains(&i) { 0.0 } else { 3000.0 + (i % 3) as f64 * 500.0 });
    let mean = data.iter().map(|p| p.kbps).sum::<f64>() / 360.0;
    reply(&rig.control.history, history(data.clone()));

    let out = run(&rig, json!({ "minutes": 60, "mesure": "debit" }));

    let card = &out["carte"];
    assert_eq!((card["kind"].as_str(), card["style"].as_str(), card["unit"].as_str(), card["title"].as_str()), (Some("chart"), Some("line"), Some("kb/s"), Some("Débit du flux (1 h)")));
    assert_eq!(card["points"].as_array().unwrap().len(), 24);
    assert_eq!(validate_card(card).as_ref(), Some(card));
    assert_eq!((out["resume"]["moyenne"].clone(), out["resume"]["minimum"].clone()), (json!(mean.round()), json!(0.0)));
    assert_eq!((out["resume"]["nb_episodes"].clone(), out["resume"]["episodes"][0]["duree_s"].clone()), (json!(1), json!(30)));
    assert_eq!((out["resume"]["chaines"].clone(), out["resume"]["redemarrages"].clone()), (json!(["TF1"]), json!(0)));
    assert_eq!(card["points"][0]["value"], json!((data[..15].iter().map(|p| p.kbps).sum::<f64>() / 15.0).round()));
    assert_eq!(*rig.control.history_limits.lock().unwrap(), vec![360]);
}

#[test]
fn stutter_uses_the_loss_unit_and_reports_the_maximum() {
    let rig = Rig::new();
    let mut data = points(60, |_| 4000.0);
    for p in &mut data[30..36] {
        p.loss = 4.0;
        p.health = Some("degraded".into());
    }
    reply(&rig.control.history, history(data));

    let out = run(&rig, json!({ "minutes": 10, "mesure": "saccades" }));

    let card = &out["carte"];
    assert_eq!((card["unit"].as_str(), card["title"].as_str()), (Some("s perdue/min"), Some("Saccades du flux (10 min)")));
    assert_eq!(validate_card(card).as_ref(), Some(card));
    assert_eq!((out["resume"]["maximum"].clone(), out["resume"]["nb_episodes"].clone()), (json!(4.0), json!(1)));
}

#[test]
fn a_few_points_are_not_aggregated_beyond_their_number() {
    let rig = Rig::new();
    reply(&rig.control.history, history(points(10, |_| 4000.0)));

    let out = run(&rig, json!({ "minutes": 5 }));

    assert_eq!(out["carte"]["points"].as_array().unwrap().len(), 10);
}

#[test]
fn a_short_history_an_unknown_measure_and_a_dead_live_return_an_error_and_no_invented_card() {
    let rig = Rig::new();
    reply(&rig.control.history, history(points(3, |_| 4000.0)));
    let short = run(&rig, json!({ "minutes": 60 }));
    reply(&rig.control.history, history(points(50, |_| 4000.0)));
    let unknown = run(&rig, json!({ "mesure": "couleur" }));
    fail(&rig.control.history, "boom http://iptv.test/live/TESTUSER/TESTPASS42/1.ts");

    let down = run(&rig, json!({}));

    assert!(short.get("carte").is_none() && short["erreur"].as_str().unwrap().contains("trop court"));
    assert!(unknown["erreur"].is_string() && unknown.get("carte").is_none());
    assert!(down["erreur"].is_string() && down.get("carte").is_none());
}

#[test]
fn the_window_is_bounded_to_three_hours_and_defaults_to_one() {
    let rig = Rig::new();
    reply(&rig.control.history, history(points(50, |_| 4000.0)));

    run(&rig, json!({ "minutes": 100_000 }));
    run(&rig, json!({ "minutes": 0 }));

    assert_eq!(*rig.control.history_limits.lock().unwrap(), vec![1080, 360]);
}

#[test]
fn standby_is_never_an_outage() {
    let rig = Rig::new();
    let mut data: Vec<HistoryPoint> = (0..6).map(|i| HistoryPoint { paused: true, ..point(i, 0.0) }).collect();
    reply(&rig.control.history, history(data.clone()));
    let all_standby = run(&rig, json!({ "minutes": 5 }));
    data.extend((6..12).map(|i| point(i, 4000.0)));
    reply(&rig.control.history, history(data));

    let mixed = run(&rig, json!({ "minutes": 5 }));

    assert!(all_standby["erreur"].as_str().unwrap().contains("veille"));
    assert_eq!((mixed["resume"]["nb_episodes"].clone(), mixed["resume"]["moyenne"].clone(), mixed["resume"]["veille_s"].clone()), (json!(0), json!(4000.0), json!(60)));
}

#[test]
fn an_isolated_bad_point_is_not_an_episode_but_two_are() {
    let rig = Rig::new();
    let shape = [4000.0, 0.0, 4000.0, 0.0, 0.0, 4000.0];
    reply(&rig.control.history, history(shape.iter().enumerate().map(|(i, k)| point(i as i64, *k)).collect()));

    let out = run(&rig, json!({ "minutes": 5 }));

    assert_eq!((out["resume"]["nb_episodes"].clone(), out["resume"]["episodes"][0]["duree_s"].clone()), (json!(1), json!(20)));
}

#[test]
fn the_tool_is_hidden_once_the_live_module_is_known_not_to_offer_the_history() {
    let rig = Rig::new();

    let out = run(&rig, json!({}));

    assert!(out["erreur"].as_str().unwrap().contains("indisponible"));
    assert!(!rig.deps.supported("history", rig.now()));
}
