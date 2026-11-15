use serde_json::json;

use super::scan_support::remember_scan;
use super::support::{call, fail, reply, run_left};
use crate::peers::{Candidate, Probe};
use crate::testing::{Rig, Scripted};

fn report(extra: impl FnOnce(&mut Probe)) -> Probe {
    let mut probe = Probe { channel_id: "m6".into(), name: "M6".into(), provider: Some("primary".into()), alive: true, resolution: Some("1920x1080".into()), codec: Some("H264".into()), audio_codec: Some("AAC".into()), fps: Some(25.0), bitrate_kbps: Some(5120.0), ..Probe::default() };
    extra(&mut probe);
    probe
}

fn backup_channel(rig: &Rig) {
    rig.control.channels.lock().unwrap().push(Candidate { id: "s:3".into(), name: "|FR| M6 HD".into(), group: "FR".into(), score: 5.0, now: None, provider: "backup".into(), event: None });
}

#[test]
fn a_living_channel_is_described_with_its_codec_resolution_rate_and_audio() {
    let rig = Rig::new();
    reply(&rig.control.probe, report(|_| {}));

    let out = call(&rig, "probe_channel", json!({ "query": "m6" }), "Mamie");

    assert_eq!((out["chaine"].as_str(), out["ref"].as_str(), out["vivante"].as_bool(), out["codec"].as_str(), out["resolution"].as_str()), (Some("M6"), Some("m6"), Some(true), Some("H264"), Some("1920x1080")));
    assert_eq!((out["fps"].clone(), out["debit_kbps"].clone(), out["mbps_mesure"].clone(), out["codec_audio"].clone()), (json!(25.0), json!(5120.0), json!(5.1), json!("AAC")));
    assert_eq!(*rig.control.probe_ids.lock().unwrap(), vec!["m6".to_string()]);
}

#[test]
fn the_channel_on_air_is_measured_locally_and_says_so() {
    let rig = Rig::new();
    reply(&rig.control.probe, report(|p| p.local = true));

    let out = call(&rig, "probe_channel", json!({ "query": "tf1" }), "Mamie");

    assert!(out["note"].as_str().unwrap().contains("mesure locale"));
}

#[test]
fn every_error_code_of_the_probe_point_comes_back_with_its_meaning() {
    for (code, etat) in [("busy", "occupe"), ("liveAtThisProvider", "refus"), ("standby", "veille"), ("notReady", "indisponible")] {
        let rig = Rig::new();
        reply(&rig.control.probe, report(|p| {
            p.alive = false;
            p.error = Some(code.into());
        }));

        let out = call(&rig, "probe_channel", json!({ "query": "m6" }), "Mamie");

        assert_eq!(out["etat"].as_str(), Some(etat), "{code}");
        assert!(out["raison"].as_str().unwrap().len() > 15, "{code}");
    }
}

#[test]
fn a_busy_probe_is_explicitly_not_a_dead_channel() {
    let rig = Rig::new();
    reply(&rig.control.probe, report(|p| {
        p.alive = false;
        p.error = Some("busy".into());
    }));

    let out = call(&rig, "probe_channel", json!({ "query": "m6" }), "Mamie");

    assert!(out["raison"].as_str().unwrap().contains("PAS une chaîne morte"));
}

#[test]
fn a_channel_that_does_not_answer_is_reported_dead_with_its_off_air_flag() {
    let rig = Rig::new();
    reply(&rig.control.probe, report(|p| {
        p.alive = false;
        p.off_air = true;
    }));

    let out = call(&rig, "probe_channel", json!({ "query": "m6" }), "Mamie");

    assert_eq!((out["etat"].as_str(), out["vivante"].as_bool(), out["hors_antenne"].as_bool()), (Some("ne_repond_pas"), Some(false), Some(true)));
}

#[test]
fn an_unknown_channel_or_reference_is_an_error_with_no_probe_sent() {
    let rig = Rig::new();

    let by_name = call(&rig, "probe_channel", json!({ "query": "chaine inconnue" }), "Mamie");
    let by_ref = call(&rig, "probe_channel", json!({ "query": "#7.1" }), "Mamie");

    assert!(by_name["error"].as_str().unwrap().contains("introuvable"));
    assert!(by_ref["error"].as_str().unwrap().contains("référence inconnue"));
    assert!(rig.control.probe_ids.lock().unwrap().is_empty());
}

#[test]
fn a_backup_reference_and_a_scan_group_reference_are_probed_by_their_id() {
    let rig = Rig::new();
    backup_channel(&rig);
    remember_scan(&rig, 5, "s:3");
    reply(&rig.control.probe, report(|p| p.provider = Some("backup".into())));

    let direct = call(&rig, "probe_channel", json!({ "query": "s:3" }), "Mamie");
    let grouped = call(&rig, "probe_channel", json!({ "query": "#5.1" }), "Mamie");

    assert_eq!((direct["fournisseur"].as_str(), grouped["ref"].as_str()), (Some("secours"), Some("s:3")));
    assert_eq!(*rig.control.probe_ids.lock().unwrap(), vec!["s:3".to_string(), "s:3".to_string()]);
}

#[test]
fn a_name_is_searched_in_the_catalogue_of_the_requested_provider() {
    let rig = Rig::new();
    backup_channel(&rig);
    reply(&rig.control.probe, report(|_| {}));

    call(&rig, "probe_channel", json!({ "query": "m6", "fournisseur": "secours" }), "Mamie");

    assert_eq!(rig.control.resolve_calls.lock().unwrap()[0].1.as_deref(), Some("backup"));
    assert_eq!(rig.control.probe_ids.lock().unwrap()[0], "s:3");
}

#[test]
fn a_live_module_without_the_probe_point_gives_an_error_and_hides_the_tool() {
    let rig = Rig::new();
    fail(&rig.control.probe, "HTTP 404: not found");

    let out = call(&rig, "probe_channel", json!({ "query": "m6" }), "Mamie");

    assert!(out["error"].as_str().unwrap().contains("sonde impossible"));
    assert!(!rig.deps.supported("probe", rig.now()));
}

#[test]
fn a_turn_with_too_little_time_left_refuses_a_probe() {
    let rig = Rig::new();
    reply(&rig.control.probe, report(|_| {}));

    let (out, _) = run_left(&rig, &Scripted::new(Vec::new()), "probe_channel", json!({ "query": "m6" }), "Mamie", Some(30.0));

    assert_eq!(out["status"], "refus");
    assert!(rig.control.probe_ids.lock().unwrap().is_empty());
}
