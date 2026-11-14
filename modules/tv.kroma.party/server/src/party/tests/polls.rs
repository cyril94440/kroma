use serde_json::json;

use super::support::*;

fn open_yesno(rig: &Rig) {
    rig.party.start_poll("Match ?", "yesno", &[], None).unwrap();
}

#[test]
fn everyone_present_having_voted_closes_the_poll_with_a_summary_in_the_chat() {
    let rig = rig();
    rig.present(&["Mamie", "Papi"]);
    open_yesno(&rig);
    rig.party.vote_poll(&person("Mamie"), " OUI ").unwrap();
    rig.party.vote_poll(&person("Papi"), "\u{1F44E}").unwrap();
    assert_eq!(rig.party.vote_poll(&person("Seb"), "peut-etre"), Err("noActivePoll"));
    assert!(rig.party.lock().poll.is_none());
    let last = rig.last_system();
    assert_eq!(last.key.as_deref(), Some("party.poll.closed.everyoneVoted"));
    assert!(last.params["summary"].as_str().unwrap().contains("Oui : 1"));
    assert_eq!(rig.events("party.poll").last().unwrap()["closed"]["total"], 2);
}

#[test]
fn an_invalid_answer_is_refused_and_one_voice_per_person_holds() {
    let rig = rig();
    rig.present(&["Mamie", "Papi"]);
    open_yesno(&rig);
    assert_eq!(rig.party.vote_poll(&person("Seb"), "peut-etre"), Err("invalidValue"));
    rig.party.vote_poll(&person("Cyril"), "oui").unwrap();
    rig.party.vote_poll(&person("Cyril"), "non").unwrap();
    let view = rig.party.snapshot(&crate::party::Actor { person: person("Mamie"), admin: false })["poll"].clone();
    assert_eq!(view["total"], 1);
    assert_eq!(view["votes"]["cyril"], "Non");
}

#[test]
fn a_single_viewer_never_triggers_the_early_close() {
    let rig = rig();
    rig.present(&["Mamie"]);
    open_yesno(&rig);
    rig.party.vote_poll(&person("Mamie"), "oui").unwrap();
    assert!(rig.party.lock().poll.is_some());
}

#[test]
fn a_poll_can_be_closed_and_the_last_one_is_kept_for_the_assistant() {
    let rig = rig();
    rig.present(&["Mamie", "Papi"]);
    rig.party.start_poll("Qui ?", "choice", &["Lens".into(), "Paris".into()], None).unwrap();
    rig.party.vote_poll(&person("A"), "lens").unwrap();
    assert_eq!(rig.party.vote_poll(&person("B"), "Nice"), Err("invalidValue"));
    rig.party.close_poll("closedByAgent").unwrap();
    rig.party.start_poll("Score ?", "score", &[], None).unwrap();
    rig.party.vote_poll(&person("A"), "2-1").unwrap();
    assert_eq!(rig.party.vote_poll(&person("B"), "__proto__"), Err("invalidValue"));
    let closed = rig.party.close_poll("closedByAdmin").unwrap();
    assert_eq!(closed.total, 1);
    let state = rig.party.snapshot(&crate::party::Actor { person: person("Mamie"), admin: false });
    assert!(state["poll"].is_null());
    assert_eq!(state["lastPoll"]["question"], "Score ?");
    assert_eq!(rig.party.close_poll("closedByAdmin"), Err("noActivePoll"));
}

#[test]
fn invalid_polls_and_a_second_poll_are_refused_with_the_source_codes() {
    let rig = rig();
    assert_eq!(rig.party.start_poll("", "yesno", &[], None).unwrap_err(), "invalidQuestionOrType");
    assert_eq!(rig.party.start_poll("x", "choice", &["seul".into()], None).unwrap_err(), "choiceNeedsTwoOptions");
    let view = rig.party.start_poll("Ok ?", "yesno", &[], Some(5)).unwrap();
    assert_eq!(view.ends_in_s, 20);
    assert_eq!(rig.party.start_poll("Autre", "yesno", &[], None).unwrap_err(), "pollAlreadyActive");
}

#[test]
fn a_poll_expires_with_the_clock_and_announces_it() {
    let rig = rig();
    open_yesno(&rig);
    rig.advance(121_000);
    rig.party.tick();
    assert!(rig.party.lock().poll.is_none());
    assert_eq!(rig.last_system().key.as_deref(), Some("party.poll.closed.timeUp"));
}

#[test]
fn a_poll_that_expired_during_a_restart_is_closed_silently_and_kept_as_the_last_one() {
    let rig = rig();
    open_yesno(&rig);
    rig.party.vote_poll(&person("Mamie"), "oui").unwrap();
    let pool = kroma_module_sdk::host::HostStorage::store(&*rig.host).clone();
    let host: std::sync::Arc<dyn kroma_module_sdk::host::HostCtx> = rig.host.clone();
    let late = std::sync::Arc::new(|| T0 + 10 * 60_000);
    let again = crate::party::Party::with_clock(host.clone(), pool.clone(), rig.live.clone(), late);
    assert!(again.lock().poll.is_none());
    assert_eq!(again.lock().last_poll.as_ref().unwrap()["closedBecause"], "restarted");
    assert_eq!(again.lock().poll_counter, 1);
    let early = std::sync::Arc::new(|| T0 + 1000);
    let resumed = crate::party::Party::with_clock(host, pool, rig.live.clone(), early);
    assert!(resumed.lock().poll.is_none());
}

#[test]
fn a_running_poll_survives_a_restart_with_its_votes() {
    let rig = rig();
    open_yesno(&rig);
    rig.party.vote_poll(&person("Mamie"), "oui").unwrap();
    let pool = kroma_module_sdk::host::HostStorage::store(&*rig.host).clone();
    let host: std::sync::Arc<dyn kroma_module_sdk::host::HostCtx> = rig.host.clone();
    let again = crate::party::Party::with_clock(host, pool, rig.live.clone(), std::sync::Arc::new(|| T0 + 1000));
    let st = again.lock();
    assert_eq!(st.poll.as_ref().unwrap().votes["mamie"].value, "Oui");
}

#[test]
fn a_typed_answer_in_the_chat_is_counted_as_a_poll_vote() {
    let rig = rig();
    rig.present(&["Mamie", "Papi", "Ced"]);
    rig.party.start_poll("Score ?", "score", &[], None).unwrap();
    let who = crate::party::Actor { person: person("Mamie"), admin: false };
    rig.party.post_message(&who, "bonne chance").unwrap();
    assert!(rig.party.lock().poll.as_ref().unwrap().votes.is_empty());
    rig.party.post_message(&who, "2-1").unwrap();
    assert_eq!(rig.party.lock().poll.as_ref().unwrap().votes["mamie"].value, "2-1");
}

#[test]
fn the_poll_event_pins_the_json_the_clients_read() {
    let rig = rig();
    open_yesno(&rig);
    let e = rig.events("party.poll");
    let poll = &e.last().unwrap()["poll"];
    assert_eq!(poll["kind"], "yesno");
    assert_eq!(poll["options"], json!(["Oui", "Non"]));
    assert_eq!(poll["byAnswer"], json!({ "Non": [], "Oui": [] }));
    assert!(e.last().unwrap()["closed"].is_null());
}
