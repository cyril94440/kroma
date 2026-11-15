use serde_json::json;

use super::support::call;
use crate::peers::{Candidate, EventInfo};
use crate::testing::Rig;

fn backup(id: &str, name: &str) -> Candidate {
    Candidate { id: id.into(), name: name.into(), group: "FR".into(), score: 5.0, now: None, provider: "backup".into(), event: None }
}

fn with_catalogues(rig: &Rig) {
    let mut channels = rig.control.channels.lock().unwrap();
    channels.push(backup("s:4", "|FR| LIGUE1+ 4 FHD"));
    channels.push(Candidate { event: Some(EventInfo { search: "ligue1+".into(), competition: "Ligue 1".into() }), ..backup("s:5", "|FR| LIGUE1+ 5 FHD") });
}

#[test]
fn list_channels_filters_by_provider_and_labels_each_channel_with_its_provider_and_event() {
    let rig = Rig::new();
    with_catalogues(&rig);

    let out = call(&rig, "list_channels", json!({ "query": "ligue1+", "fournisseur": "secours" }), "Mamie");

    assert_eq!(out["total"], 2);
    assert_eq!((out["chaines"][1]["ref"].clone(), out["chaines"][1]["fournisseur"].clone(), out["chaines"][1]["evenement"]["recherche"].clone()), (json!("s:5"), json!("secours"), json!("ligue1+")));
    assert!(out["chaines"][0].get("evenement").is_none());
    assert_eq!(rig.control.resolve_calls.lock().unwrap()[0], ("ligue1+".to_string(), Some("backup".to_string())));
}

#[test]
fn an_unknown_provider_name_is_refused_before_any_call() {
    let rig = Rig::new();

    let out = call(&rig, "list_channels", json!({ "query": "tf1", "fournisseur": "lune" }), "Mamie");

    assert_eq!(out["ok"], false);
    assert!(rig.control.resolve_calls.lock().unwrap().is_empty());
}

#[test]
fn change_channel_searches_a_name_in_the_requested_provider_catalogue() {
    let rig = Rig::new();
    with_catalogues(&rig);

    let out = call(&rig, "change_channel", json!({ "query": "ligue1+ 4", "fournisseur": "secours" }), "Mamie");

    assert_eq!(out["status"], "vote_open");
    assert_eq!(rig.control.resolve_calls.lock().unwrap()[0].1.as_deref(), Some("backup"));
    assert_eq!(rig.room.switches.lock().unwrap()[0].0, "s:4");
}

#[test]
fn change_channel_takes_a_backup_reference_and_names_the_channel_for_the_vote() {
    let rig = Rig::new();
    with_catalogues(&rig);

    let out = call(&rig, "change_channel", json!({ "ref": "s:4" }), "Mamie");

    assert_eq!(out["status"], "vote_open");
    assert_eq!(rig.room.switch_asks.lock().unwrap()[0], (Some("|FR| LIGUE1+ 4 FHD".to_string()), false));
}

#[test]
fn force_is_forwarded_and_a_vote_instead_of_a_forced_switch_is_explained() {
    let rig = Rig::new();

    let out = call(&rig, "change_channel", json!({ "query": "m6", "force": true }), "Mamie");

    assert!(rig.room.switch_asks.lock().unwrap()[0].1);
    assert_eq!(out["status"], "vote_open");
    assert!(out["note"].as_str().unwrap().contains("force"));
}

#[test]
fn a_scan_group_reference_resolves_to_its_channel_and_a_stale_one_is_unknown() {
    let rig = Rig::new();
    with_catalogues(&rig);
    let stale = call(&rig, "change_channel", json!({ "ref": "#9.1" }), "Mamie");
    super::scan_support::remember_scan(&rig, 9, "s:4");

    let out = call(&rig, "change_channel", json!({ "ref": "#9.1" }), "Mamie");

    assert_eq!(stale["status"], "ref_inconnue");
    assert_eq!(out["status"], "vote_open");
    assert_eq!(rig.room.switches.lock().unwrap()[0].0, "s:4");
}
