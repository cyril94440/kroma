use serde_json::json;

use super::scan_support::{reading, remember, row};
use super::support::{call, fail, reply, run};
use crate::matching::Verdict;
use crate::peers::{Candidate, EventInfo, ProviderSwitch};
use crate::testing::{Rig, Scripted};

fn offered(error: Option<&str>, switched: bool, provider: &str) -> ProviderSwitch {
    ProviderSwitch { switched, provider: provider.into(), error: error.map(str::to_string) }
}

fn with_backup(rig: &Rig) {
    let mut channels = rig.control.channels.lock().unwrap();
    channels.push(Candidate { id: "s:7".into(), name: "|FR| LIGUE1+ 7".into(), group: "FR".into(), score: 5.0, now: None, provider: "backup".into(), event: None });
    channels.push(Candidate { id: "p:9".into(), name: "FR: LIGUE1+ 9".into(), group: "FR".into(), score: 5.0, now: None, provider: "primary".into(), event: None });
}

fn on_multiplex(rig: &Rig) {
    rig.control.channels.lock().unwrap()[0].event = Some(EventInfo { search: "ligue1+".into(), competition: "Ligue 1".into() });
}

#[test]
fn asking_for_the_backup_by_name_calls_the_live_point_in_the_authors_name() {
    let rig = Rig::new();
    reply(&rig.control.provider, offered(None, true, "backup"));

    let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "switch_provider", json!({ "target": "le secours" }), "Mamie");

    assert_eq!((out["status"].as_str(), out["provider"].as_str()), (Some("switched_provider"), Some("secours")));
    assert!(ctx.diffusion_done);
    assert_eq!(*rig.control.calls.lock().unwrap(), vec![("switchProvider".to_string(), "backup:u-mamie".to_string())]);
}

#[test]
fn the_other_provider_means_the_one_that_is_not_active() {
    let rig = Rig::new();
    rig.control.status.lock().unwrap().provider = "backup".into();
    reply(&rig.control.provider, offered(None, true, "primary"));

    call(&rig, "switch_provider", json!({ "target": "l'autre" }), "Mamie");

    assert_eq!(rig.control.calls.lock().unwrap()[0].1, "primary:u-mamie");
}

#[test]
fn a_vote_required_is_an_announced_outcome_like_a_channel_vote() {
    let rig = Rig::new();
    reply(&rig.control.provider, offered(Some("voteRequired"), false, "backup"));

    let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "switch_provider", json!({ "target": "secours" }), "Mamie");

    assert_eq!(out["status"], "vote_open");
    assert!(out.get("ok").is_none() && ctx.diffusion_done);
}

#[test]
fn every_refusal_of_the_provider_point_returns_a_status_and_a_reason_to_the_model() {
    for (code, status) in [("noBackup", "no_backup"), ("noChannel", "no_channel"), ("alreadyOn", "already_on"), ("primaryDown", "primary_down"), ("notFound", "not_found"), ("notWorking", "dead"), ("busy", "occupe"), ("unknownProvider", "refused")] {
        let rig = Rig::new();
        reply(&rig.control.provider, offered(Some(code), false, "backup"));

        let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "switch_provider", json!({ "target": "secours" }), "Mamie");

        assert_eq!((out["ok"].clone(), out["status"].as_str()), (json!(false), Some(status)), "{code}");
        assert!(out["raison"].as_str().unwrap().len() > 10, "{code}");
        assert!(!ctx.diffusion_done, "{code}");
    }
}

#[test]
fn a_multiplex_that_cannot_be_matched_names_the_scan_to_run_next() {
    let rig = Rig::new();
    on_multiplex(&rig);
    reply(&rig.control.provider, offered(Some("notWorking"), false, "backup"));

    let out = call(&rig, "switch_provider", json!({ "target": "secours" }), "Mamie");

    assert_eq!(out["multiplex"], true);
    assert_eq!(out["suite_possible"], "benchmark_channels(query='ligue1+', fournisseur='secours', comparer_au_direct=true)");
}

#[test]
fn a_force_request_is_told_it_does_not_exist_for_a_plain_provider_change() {
    let rig = Rig::new();
    reply(&rig.control.provider, offered(None, true, "backup"));

    let out = call(&rig, "switch_provider", json!({ "target": "secours", "force": true }), "Mamie");

    assert!(out["note_force"].as_str().unwrap().contains("force"));
}

#[test]
fn an_old_live_module_without_the_point_gives_a_failure_and_hides_the_tool() {
    let rig = Rig::new();
    fail(&rig.control.provider, "HTTP 404: not found");

    let out = call(&rig, "switch_provider", json!({ "target": "secours" }), "Mamie");

    assert_eq!(out["ok"], false);
    assert!(!rig.deps.supported("switchProvider", rig.now()));
}

#[test]
fn a_channel_of_the_other_provider_goes_through_the_live_point_with_its_reference() {
    let rig = Rig::new();
    with_backup(&rig);
    reply(&rig.control.provider, offered(Some("voteRequired"), false, "backup"));

    let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "switch_provider", json!({ "ref": "s:7" }), "Mamie");

    assert_eq!((out["status"].as_str(), out["verifie_a_l_image"].as_bool()), (Some("vote_open"), Some(false)));
    assert!(ctx.diffusion_done);
    let asks = rig.control.provider_asks.lock().unwrap();
    assert_eq!((asks[0].0.as_str(), asks[0].1.as_deref()), ("backup", Some("s:7")));
    assert!(asks[0].2.contains("|FR| LIGUE1+ 7") && asks[0].2.contains("non vérifié"));
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn a_channel_of_the_other_provider_switched_at_once_is_a_switched_provider() {
    let rig = Rig::new();
    with_backup(&rig);
    reply(&rig.control.provider, offered(None, true, "backup"));

    let out = call(&rig, "switch_provider", json!({ "ref": "s:7", "force": true }), "Mamie");

    assert_eq!((out["status"].as_str(), out["provider"].as_str()), (Some("switched_provider"), Some("secours")));
    assert!(out["note_force"].as_str().unwrap().contains("force"));
}

#[test]
fn the_refusals_that_name_a_channel_reach_the_model_with_a_status_and_a_reason() {
    for (code, status) in [("unknownChannel", "ref_inconnue"), ("providerMismatch", "mauvais_fournisseur"), ("notWorking", "dead"), ("notFound", "not_found"), ("busy", "occupe")] {
        let rig = Rig::new();
        with_backup(&rig);
        reply(&rig.control.provider, offered(Some(code), false, "backup"));

        let (out, ctx) = run(&rig, &Scripted::new(Vec::new()), "switch_provider", json!({ "ref": "s:7" }), "Mamie");

        assert_eq!((out["ok"].clone(), out["status"].as_str()), (json!(false), Some(status)), "{code}");
        assert!(out["raison"].as_str().unwrap().len() > 10 && !ctx.diffusion_done, "{code}");
        assert!(rig.room.switches.lock().unwrap().is_empty(), "{code}");
    }
}

#[test]
fn a_live_module_without_the_point_still_gets_the_reference_through_the_rooms_vote() {
    let rig = Rig::new();
    with_backup(&rig);
    fail(&rig.control.provider, "HTTP 404: not found");

    let out = call(&rig, "switch_provider", json!({ "ref": "s:7" }), "Mamie");

    assert_eq!(out["status"], "vote_open");
    assert_eq!(rig.room.switch_asks.lock().unwrap()[0].0.as_deref(), Some("|FR| LIGUE1+ 7"));
    assert!(!rig.deps.supported("switchProvider", rig.now()));
}

#[test]
fn a_channel_of_the_active_provider_is_a_channel_change_not_a_provider_change() {
    let rig = Rig::new();
    with_backup(&rig);

    let out = call(&rig, "switch_provider", json!({ "ref": "p:9" }), "Mamie");

    assert_eq!((out["status"].as_str(), out["suite_possible"].as_str()), (Some("meme_fournisseur"), Some("change_channel(ref='p:9')")));
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn an_unknown_reference_is_not_requested() {
    let rig = Rig::new();

    let out = call(&rig, "switch_provider", json!({ "ref": "#4.1" }), "Mamie");

    assert_eq!(out["status"], "ref_inconnue");
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn a_channel_a_recent_scan_proved_to_show_another_match_is_refused() {
    let rig = Rig::new();
    with_backup(&rig);
    remember(&rig, 3, "tf1", true, vec![row(0, "s:7", Some(reading("direct", "Lyon", "Monaco", "0-0", "33:10")), Some(Verdict::No))]);

    let out = call(&rig, "switch_provider", json!({ "ref": "s:7" }), "Mamie");

    assert_eq!((out["status"].as_str(), out["suite_possible"].as_str()), (Some("autre_match"), Some("change_channel(ref='s:7')")));
    assert!(rig.room.switches.lock().unwrap().is_empty());
}

#[test]
fn a_no_is_not_held_against_a_channel_when_the_live_picture_changed_since_the_scan() {
    let rig = Rig::new();
    with_backup(&rig);
    remember(&rig, 3, "tf1", true, vec![row(0, "s:7", Some(reading("direct", "Lyon", "Monaco", "0-0", "33:10")), Some(Verdict::No))]);
    rig.advance(1_000);
    rig.deps.live_seen.set("tf1", &reading("direct", "Marseille", "Nice", "1-1", "50:00"), rig.now());
    reply(&rig.control.provider, offered(Some("voteRequired"), false, "backup"));

    let out = call(&rig, "switch_provider", json!({ "ref": "s:7" }), "Mamie");

    assert_eq!(out["status"], "vote_open");
}

#[test]
fn a_channel_a_recent_scan_proved_to_show_the_same_match_is_voted_as_verified() {
    let rig = Rig::new();
    with_backup(&rig);
    remember(&rig, 3, "tf1", true, vec![row(0, "s:7", Some(reading("direct", "Lens", "Paris FC", "1-0", "34:12")), Some(Verdict::Yes))]);
    reply(&rig.control.provider, offered(Some("voteRequired"), false, "backup"));

    let out = call(&rig, "switch_provider", json!({ "ref": "#3.1" }), "Mamie");

    assert_eq!((out["status"].as_str(), out["verifie_a_l_image"].as_bool()), (Some("vote_open"), Some(true)));
    let asks = rig.control.provider_asks.lock().unwrap();
    assert_eq!(asks[0].1.as_deref(), Some("s:7"));
    assert!(asks[0].2.contains("même match vérifié à l'image"));
}
