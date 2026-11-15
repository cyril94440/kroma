use serde_json::{json, Value};

use super::support::{at, Rig};
use crate::espn::scoreboard;
use crate::peers::{Candidate, Programme, ScanReport};
use crate::sport::{reading_from_title, ScanRow};

fn payload(state: &str, score: (&str, &str)) -> Value {
    let mut p: Value =
        serde_json::from_str(include_str!("../../../fixtures/espn_sport_foot.json")).unwrap();
    let competition = &mut p["events"][0]["competitions"][0];
    competition["status"]["type"]["state"] = state.into();
    if state == "in" {
        competition["status"]["displayClock"] = "30'".into();
    }
    competition["competitors"][0]["score"] = score.0.into();
    competition["competitors"][1]["score"] = score.1.into();
    p
}

fn serve_league(rig: &Rig, state: &str, score: (&str, &str)) {
    for day in ["20261105", "20261106"] {
        rig.espn
            .serve(&scoreboard("fra.1", day), &payload(state, score));
    }
}

fn programme(channel: &str, name: &str) -> Programme {
    Programme {
        channel_id: channel.into(),
        channel_name: name.into(),
        title: "Toulouse / Strasbourg".into(),
        description: String::new(),
        start: (at("2026-11-06T19:30:00Z") * 1000.0) as i64,
        end: (at("2026-11-06T22:00:00Z") * 1000.0) as i64,
    }
}

fn first_event(rig: &Rig) -> Option<Value> {
    rig.events("sport.view").into_iter().rev().find_map(|v| {
        v["view"]["evenements"]
            .as_array()
            .and_then(|e| e.first().cloned())
    })
}

#[test]
fn the_scoreboards_are_crossed_with_the_guide_and_published_with_a_sequence() {
    let rig = Rig::new("2026-11-06T18:30:00Z");
    serve_league(&rig, "pre", ("0", "0"));
    *rig.live.programmes.lock().unwrap() = vec![programme("primary:l1p1", "FR: LIGUE1+ 1 RAW")];

    let found = rig.run_until(200, |r| {
        first_event(r).is_some_and(|e| e["chaine"].is_object())
    });

    let event = first_event(&rig).unwrap();
    assert!(found);
    assert_eq!(
        (
            event["id"].as_str(),
            event["compet"].as_str(),
            event["statut"].as_str(),
            event["ligue1"].clone()
        ),
        (
            Some("foot:401876405"),
            Some("Ligue 1"),
            Some("pre"),
            json!(true)
        )
    );
    assert_eq!(
        (
            event["chaine"]["nom"].as_str(),
            event["chaine"]["ref"].as_str(),
            event["chaine"]["source"].as_str(),
            event["chaine"]["certitude"].as_str()
        ),
        (
            Some("FR: LIGUE1+ 1 RAW"),
            Some("primary:l1p1"),
            Some("guide"),
            Some("guide")
        )
    );
    assert_eq!(event["lignes"][0]["score"], "");
    assert!(rig.events("sport.view").iter().all(|v| v["seq"].is_u64()));
    assert!(rig
        .espn
        .calls()
        .iter()
        .any(|c| c.contains("soccer/fra.1/scoreboard?dates=20261106")));
}

#[test]
fn opening_the_view_starts_a_scan_and_its_result_names_the_channel_to_confirm() {
    let rig = Rig::new("2026-11-06T19:15:00Z");
    serve_league(&rig, "in", ("1", "0"));
    rig.run_until(200, |r| first_event(r).is_some());
    *rig.live.channels.lock().unwrap() = vec![Candidate {
        id: "primary:l1p3".into(),
        name: "FR: LIGUE1+ 3 RAW".into(),
        group: "FR".into(),
        score: 9.0,
    }];
    *rig.live.scan.lock().unwrap() = Some(ScanReport {
        running: false,
        total: 1,
        rows: vec![ScanRow {
            channel_id: "primary:l1p3".into(),
            name: Some("FR: LIGUE1+ 3 RAW".into()),
            num: Some(3),
            state: "ok".into(),
            reading: reading_from_title("Ligue 1 : Toulouse - Strasbourg"),
            from_guide: true,
        }],
    });

    let first = rig.sport.open_view("u-ana", true);
    let limited = rig.sport.open_view("u-ana", true);
    let seen = rig.run_until(60, |r| {
        first_event(r).is_some_and(|e| e["chaine"]["source"] == "image")
    });

    let event = first_event(&rig).unwrap();
    let view = rig.events("sport.view").pop().unwrap()["view"].clone();
    assert_eq!((first, limited), (true, false));
    assert_eq!(
        rig.live.benchmarks.lock().unwrap().as_slice(),
        [vec!["primary:l1p3".to_string()]]
    );
    assert!(seen);
    assert_eq!(
        (
            event["chaine"]["ref"].as_str(),
            event["chaine"]["certitude"].as_str()
        ),
        (Some("primary:l1p3"), Some("a_confirmer"))
    );
    assert_eq!(
        (
            view["ligue1"]["canaux"][0]["num"].clone(),
            view["ligue1"]["canaux"][0]["certitude"].clone()
        ),
        (json!(3), json!("a_confirmer"))
    );
    assert_eq!(view["scan"]["etat"], "fait");
}

#[test]
fn a_scan_the_live_module_cannot_do_yet_is_reported_as_impossible() {
    let rig = Rig::new("2026-11-06T19:15:00Z");
    serve_league(&rig, "in", ("1", "0"));
    rig.run_until(200, |r| first_event(r).is_some());
    *rig.live.scan.lock().unwrap() = None;

    rig.sport.open_view("u-ana", true);
    rig.run(10);

    let view = rig.events("sport.view").pop().unwrap()["view"].clone();
    assert_eq!(view["scan"]["etat"], "impossible");
}
