use serde_json::json;

use super::support::{run, run_left};
use crate::model::ScreenReading;
use crate::peers::{ChannelRef, Snapshot, Status};
use crate::testing::{Rig, Scripted};

fn frame(rig: &Rig) {
    *rig.control.snapshot.lock().unwrap() = Some(Ok(Snapshot { jpeg_base64: "QUJD".into(), at: rig.now() - 2000 }));
}

fn trusted(team1: &str, team2: &str) -> Scripted {
    let model = Scripted::new(Vec::new());
    let mut reading = ScreenReading::unknown();
    reading.kind = "direct".into();
    reading.team1 = team1.into();
    reading.team2 = team2.into();
    reading.score = "1-0".into();
    reading.clock = "34:12".into();
    reading.confidence = 0.9;
    *model.0.screen.lock().unwrap() = Some(Ok(reading));
    model
}

#[test]
fn every_refusal_of_the_snapshot_point_comes_back_as_a_reason_the_model_can_tell() {
    for (code, etat) in [("standby", "veille"), ("stopped", "arrete"), ("settling", "indisponible"), ("changed", "indisponible"), ("captureFailed", "indisponible"), ("tooLarge", "indisponible")] {
        let rig = Rig::new();
        *rig.control.snapshot.lock().unwrap() = Some(Err(code.into()));

        let (out, _) = run(&rig, &Scripted::new(Vec::new()), "look_at_live", json!({}), "Mamie");

        assert_eq!((out["ok"].clone(), out["etat"].as_str(), out["raison"].as_str(), out["chaine"].as_str()), (json!(false), Some(etat), Some(code), Some("TF1")), "{code}");
        assert!(out["note"].as_str().unwrap().len() > 10, "{code}");
        assert!(rig.deps.supported("snapshot", rig.now()), "{code}");
    }
}

#[test]
fn a_reading_carries_the_source_fields_the_channel_and_the_freeze_flag() {
    let rig = Rig::new();
    frame(&rig);
    rig.control.status.lock().unwrap().health.frozen = true;

    let (out, _) = run(&rig, &trusted("Lens", "PSG"), "look_at_live", json!({ "question": "score ?" }), "Mamie");

    assert_eq!((out["chaine"].as_str(), out["fournisseur"].as_str(), out["gratuit"].as_bool(), out["figee"].as_bool()), (Some("TF1"), Some("principal"), Some(true), Some(true)));
    assert_eq!((out["type"].as_str(), out["equipes"].clone(), out["score"].as_str(), out["horloge"].as_str()), (Some("direct"), json!(["Lens", "PSG"]), Some("1-0"), Some("34:12")));
    assert_eq!((out["lu_il_y_a_s"].clone(), out["confiance"].clone(), out["fiable"].clone()), (json!(2), json!(0.9), json!(true)));
}

#[test]
fn a_reading_with_teams_is_remembered_as_the_last_picture_of_that_channel() {
    let rig = Rig::new();
    frame(&rig);

    run(&rig, &trusted("Lens", "PSG"), "look_at_live", json!({}), "Mamie");

    assert!(rig.deps.live_seen.teams("tf1", rig.now(), 600_000).is_some());
    assert!(rig.deps.live_seen.teams("m6", rig.now(), 600_000).is_none());
}

#[test]
fn a_channel_that_changed_during_the_reading_gives_no_reading() {
    let rig = Rig::new();
    frame(&rig);
    *rig.control.status_after_snapshot.lock().unwrap() = Some(Status { channel: Some(ChannelRef { id: "m6".into(), name: "M6".into() }), ..rig.control.status.lock().unwrap().clone() });

    let (out, _) = run(&rig, &trusted("Lens", "PSG"), "look_at_live", json!({}), "Mamie");

    assert_eq!((out["ok"].clone(), out["raison"].as_str()), (json!(false), Some("changed")));
    assert!(rig.deps.live_seen.teams("tf1", rig.now(), 600_000).is_none());
}

#[test]
fn a_turn_with_less_than_a_minute_left_is_not_given_a_capture() {
    let rig = Rig::new();
    frame(&rig);

    let (out, _) = run_left(&rig, &trusted("Lens", "PSG"), "look_at_live", json!({}), "Mamie", Some(40.0));

    assert_eq!(out["status"], "refus");
    assert!(out["raison"].as_str().unwrap().contains("regarder l'image"));
}
