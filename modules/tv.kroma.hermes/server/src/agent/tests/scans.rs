use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::loops::{calls, fin, play, result_of};
use crate::peers::{BenchmarkStart, Candidate, ProviderSwitch, ScanItem, ScanState, Snapshot};
use crate::testing::{Rig, Scripted};

pub fn backup(id: &str, name: &str) -> Candidate {
    Candidate { id: id.into(), name: name.into(), group: "FR".into(), score: 5.0, now: None, provider: "backup".into(), event: None }
}

pub fn item(number: u32, id: &str, black: bool, error: Option<&str>) -> ScanItem {
    let ok = !black && error.is_none();
    ScanItem { number: Some(number), id: id.into(), name: format!("|FR| LIGUE1+ {number}"), ok, black, still: false, frame: ok.then(|| format!("scan-1/{number}.jpg")), frames: if ok { vec![format!("scan-1/{number}.jpg")] } else { Vec::new() }, error: error.map(str::to_string), now: None }
}

pub fn state(running: bool, items: Vec<ScanItem>) -> ScanState {
    ScanState { id: "1".into(), query: "ligue1+".into(), running, provider: Some("backup".into()), total: items.len().max(2), items }
}

pub fn rig_with_backup() -> Rig {
    let rig = Rig::new();
    let mut channels = rig.control.channels.lock().unwrap();
    for n in 1..=4 {
        channels.push(backup(&format!("s:{n}"), &format!("|FR| LIGUE1+ {n}")));
    }
    drop(channels);
    *rig.control.benchmark.lock().unwrap() = Some(Ok(BenchmarkStart { status: "started".into(), id: Some("1".into()), reason: None, total: 4, already_done: false }));
    rig
}

pub fn frames(rig: &Rig, numbers: &[u32]) {
    let map: BTreeMap<String, String> = numbers.iter().map(|n| (format!("scan-1/{n}.jpg"), format!("jpeg-{n}"))).collect();
    *rig.control.frames.lock().unwrap() = Some(Ok(map));
}

pub fn live_picture(rig: &Rig) {
    *rig.control.snapshot.lock().unwrap() = Some(Ok(Snapshot { jpeg_base64: "QUJD".into(), at: rig.now() - 1000 }));
}

pub fn readings(model: &Scripted, list: Vec<crate::model::ScreenReading>) {
    model.0.screens.lock().unwrap().extend(list.into_iter().map(Ok));
}

pub fn direct(team1: &str, team2: &str, score: &str, clock: &str) -> crate::model::ScreenReading {
    let mut r = crate::model::ScreenReading::unknown();
    r.kind = "direct".into();
    r.team1 = team1.into();
    r.team2 = team2.into();
    r.score = score.into();
    r.clock = clock.into();
    r.confidence = 0.9;
    r
}

fn bench(args: Value) -> (&'static str, Value) {
    ("benchmark_channels", args)
}

#[test]
fn a_scan_is_followed_across_ticks_and_its_groups_come_back_to_the_model() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(true, Vec::new()), state(false, vec![item(1, "s:1", false, None), item(2, "s:2", true, None), item(3, "s:3", false, Some("HTTP 503"))])];
    frames(&rig, &[1]);
    let model = Scripted::new(vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "Voilà." }))])]);
    readings(&model, vec![direct("Lens", "Paris FC", "1-0", "34:12")]);
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "scanne les ligue1+ du secours");
    rig.drain();

    let out = result_of(&model, 0, 0);
    assert_eq!((out["scan"].as_str(), out["fournisseur"].as_str(), out["scan_n"].clone()), (Some("fait"), Some("secours"), json!(1)));
    assert_eq!((out["groupes"][0]["ref"].as_str(), out["groupes"][0]["contenu"].as_str()), (Some("#1.1"), Some("direct — Lens – Paris FC, 1-0, 34:12")));
    assert_eq!((out["sans_image"]["noires"].clone(), out["sans_image"]["sans_reponse"].clone()), (json!(1), json!(1)));
    assert_eq!(out["sans_image"]["detail"][1]["raison"], "erreur du portail (5XX)");
    assert_eq!(*rig.control.benchmark_ids.lock().unwrap(), vec![vec!["s:1".to_string(), "s:2".into(), "s:3".into(), "s:4".into()]]);
    assert_eq!(fin(&rig), "terminal");
}

#[test]
fn the_scan_is_announced_once_and_not_when_the_model_already_posted_a_waiting_message() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", true, None)])];
    play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours" })), ("reply", json!({ "message": "Je scanne…" }))]), calls(vec![("reply", json!({ "message": "Rien." }))])]);

    let texts = rig.room.texts();

    assert_eq!(texts, vec!["Je scanne…".to_string(), "Rien.".to_string()]);
}

#[test]
fn without_a_waiting_message_the_system_announces_the_scan_with_a_duration_range() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", true, None)])];
    play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "comparer_au_direct": true }))]), calls(vec![("reply", json!({ "message": "Rien." }))])]);

    let first = rig.room.texts()[0].clone();

    assert!(first.starts_with("🔎 Je scanne 4 chaîne(s) « ligue1+ » chez le fournisseur secours, et je les compare au direct"), "{first}");
    assert!(first.ends_with("Le direct n'est pas touché."));
}

#[test]
fn at_most_two_scans_per_message() {
    let rig = rig_with_backup();
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", true, None)])];
    let model = play(&rig, "scanne", vec![calls(vec![bench(json!({ "query": "ligue1+ 1", "fournisseur": "secours" }))]), calls(vec![bench(json!({ "query": "ligue1+ 2", "fournisseur": "secours" }))]), calls(vec![bench(json!({ "query": "ligue1+ 3", "fournisseur": "secours" }))]), calls(vec![("reply", json!({ "message": "fini" }))])]);

    assert_eq!(rig.control.benchmark_ids.lock().unwrap().len(), 2);
    assert!(result_of(&model, 2, 0)["raison"].as_str().unwrap().contains("deux scans maximum"));
}

#[test]
fn a_refused_provider_then_a_scan_then_the_right_channel_ends_in_one_vote() {
    let rig = rig_with_backup();
    *rig.control.provider.lock().unwrap() = Some(Ok(ProviderSwitch { switched: false, provider: "backup".into(), error: Some("notWorking".into()) }));
    *rig.control.scans.lock().unwrap() = vec![state(false, vec![item(1, "s:1", false, None)])];
    frames(&rig, &[1]);
    live_picture(&rig);
    let model = Scripted::new(vec![
        calls(vec![("switch_provider", json!({ "target": "secours" }))]),
        calls(vec![bench(json!({ "query": "ligue1+", "fournisseur": "secours", "comparer_au_direct": true }))]),
        calls(vec![("switch_provider", json!({ "ref": "#1.1" }))]),
    ]);
    readings(&model, vec![direct("Lens", "Paris FC", "1-0", "34:12"), direct("Lens", "Paris FC", "1-0", "34:20")]);
    rig.use_model(std::sync::Arc::new(model.clone()));

    rig.hear("m-1", "Mamie", "passe sur le secours");
    rig.drain();

    assert_eq!(fin(&rig), "terminal");
    assert_eq!(result_of(&model, 0, 0)["status"], "dead", "the failure comes back to the model instead of closing the turn");
    assert_eq!(result_of(&model, 1, 0)["groupes"][0]["meme_que_direct"], "oui");
    assert_eq!(rig.room.switches.lock().unwrap().len(), 1);
    assert_eq!(rig.room.switches.lock().unwrap()[0].0, "s:1");
    assert!(rig.room.switches.lock().unwrap()[0].2.contains("même match vérifié à l'image"));
}
