use std::collections::BTreeSet;

use serde_json::json;

use crate::agent::gate::{decide, Room, Signals, Verdict};
use crate::testing::{Rig, Scripted, Step};

fn names() -> Vec<String> {
    vec!["hermes".into(), "hermès".into()]
}

fn verdict(text: &str, room: &Room) -> Verdict {
    decide(text, &Signals::of(text, &names(), None), "u-mamie", room)
}

#[test]
fn the_gate_matrix_follows_the_source() {
    let quiet = Room::default();
    let cases: &[(&str, Verdict)] = &[
        ("a", Verdict::Skip("too_short")),
        (" ", Verdict::Skip("too_short")),
        ("on mange quoi ce soir", Verdict::Skip("not_for_agent")),
        ("bonsoir tout le monde", Verdict::Skip("not_for_agent")),
        ("c'est bien ça ?", Verdict::Engage),
        ("mets TF1", Verdict::Engage),
        ("Hermès bonsoir", Verdict::Engage),
        ("le flux est noir", Verdict::Engage),
        ("quel est le classement de ligue 1", Verdict::Engage),
        ("la chaîne bug", Verdict::Engage),
        ("qui joue demain", Verdict::Engage),
    ];
    for (text, want) in cases {
        assert_eq!(verdict(text, &quiet), *want, "{text}");
    }
}

#[test]
fn right_after_an_answer_short_replies_without_keyword_are_engaged() {
    let talking = Room { spoke_recently: true, ..Room::default() };
    assert_eq!(verdict("Disney channel", &talking), Verdict::Engage);
    assert_eq!(verdict("a", &talking), Verdict::Skip("too_short"));
}

#[test]
fn an_open_poll_lets_every_message_through() {
    let poll = Room { poll_open: true, ..Room::default() };
    assert_eq!(verdict("l'Espagne", &poll), Verdict::Engage);
    assert_eq!(verdict("2-1", &poll), Verdict::Engage);
}

#[test]
fn a_typed_vote_from_a_waited_for_voter_belongs_to_the_vote() {
    let voting = Room { vote_required: Some(BTreeSet::from(["u-mamie".to_string()])), ..Room::default() };
    assert_eq!(verdict("oui", &voting), Verdict::Skip("vote_answer"));
    assert_eq!(verdict("non merci", &voting), Verdict::Skip("vote_answer"));
    assert_eq!(verdict("mets plutôt arte", &voting), Verdict::Engage);
    let requester = Room { vote_required: Some(BTreeSet::from(["u-papi".to_string()])), ..Room::default() };
    assert_eq!(verdict("go, teste-les", &requester), Verdict::Engage, "the requester speaking to the assistant during their own vote");
}

#[test]
fn the_party_signals_are_honoured_alongside_the_local_ones() {
    let text = "bonsoir";
    let signals = Signals::of(text, &names(), Some(&json!({ "mentionsAgent": true })));
    assert_eq!(decide(text, &signals, "u-mamie", &Room::default()), Verdict::Engage);
    assert!(Signals::of("salut Zeus", &["zeus".to_string()], None).mentions_agent);
    assert!(Signals::of("ça marche ?", &names(), None).question);
}

fn rig_with_model() -> Rig {
    let rig = Rig::new();
    rig.use_model(std::sync::Arc::new(Scripted::new((0..6).map(|_| Step::Calls(vec![("ignore", json!({}))])).collect())));
    rig
}

#[test]
fn the_agent_never_answers_itself_nor_the_system() {
    let rig = rig_with_model();
    rig.agent.on_event("party.message", &json!({ "id": "a1", "userId": null, "name": "Hermès", "text": "Je mets TF1, qu'en dis-tu ?", "at": rig.now(), "kind": "agent" }));
    rig.agent.on_event("party.message", &json!({ "id": "s1", "name": "", "text": "Papi a rejoint, mets TF1 ?", "at": rig.now(), "kind": "system" }));
    rig.drain();
    assert!(rig.deps.store.inbox_recent(5).unwrap().is_empty());
    assert!(rig.agent.speaking_recently(rig.now()));
}

#[test]
fn a_message_that_does_not_concern_the_agent_is_recorded_as_skipped_without_a_model_call() {
    let rig = rig_with_model();
    rig.hear("m-1", "Mamie", "on mange quoi ce soir");
    rig.drain();
    let row = rig.deps.store.inbox_recent(5).unwrap().remove(0);
    assert_eq!((row.state.as_str(), row.reason.as_deref()), ("skipped", Some("not_for_agent")));
    assert!(rig.turns().is_empty());
    let gate = rig.deps.store.audit_rows(Some("chat"), None, 5).unwrap();
    assert_eq!(gate[0].event, "gate");
    assert_eq!(gate[0].data["verdict"], "Skip(\"not_for_agent\")");
}

#[test]
fn the_conversation_window_follows_the_last_answer_and_expires() {
    let rig = rig_with_model();
    rig.agent.on_event("party.message", &json!({ "id": "a1", "name": "Hermès", "text": "Quelle chaîne ?", "at": rig.now(), "kind": "agent" }));
    rig.hear("m-1", "Mamie", "Disney channel");
    assert_eq!(rig.deps.store.next_queued().unwrap().unwrap().message.id, "m-1");
    rig.drain();
    rig.advance(91_000);
    rig.hear("m-2", "Mamie", "Arte");
    assert_eq!(rig.deps.store.inbox_recent(1).unwrap()[0].reason.as_deref(), Some("not_for_agent"));
}

#[test]
fn vote_and_poll_events_drive_the_gate() {
    let rig = rig_with_model();
    rig.agent.on_event("party.vote", &json!({ "vote": { "label": "M6", "by": { "userId": "u-papi", "name": "Papi" }, "required": [{ "userId": "u-mamie", "name": "Mamie" }], "votes": {} } }));
    rig.hear("m-1", "Mamie", "oui");
    assert_eq!(rig.deps.store.inbox_recent(1).unwrap()[0].reason.as_deref(), Some("vote_answer"));
    rig.agent.on_event("party.vote", &json!({ "vote": null, "closed": { "verdict": "passed" } }));
    rig.hear("m-2", "Mamie", "oui");
    assert_eq!(rig.deps.store.inbox_recent(1).unwrap()[0].reason.as_deref(), Some("not_for_agent"));
    rig.agent.on_event("party.poll", &json!({ "poll": { "question": "Qui gagne ?", "kind": "choice", "options": ["Lens", "PSG"] }, "closed": null }));
    rig.hear("m-3", "Mamie", "Lens");
    assert_eq!(rig.deps.store.inbox_recent(1).unwrap()[0].state, "queued");
    rig.agent.on_event("party.poll", &json!({ "poll": null, "closed": {} }));
    rig.hear("m-4", "Mamie", "Lens");
    assert_eq!(rig.deps.store.inbox_recent(1).unwrap()[0].state, "skipped");
}

#[test]
fn a_disabled_or_paused_agent_claims_nothing_and_answers_nothing() {
    let rig = Rig::with_config(&[("enabled", json!(false))]);
    rig.hear("m-1", "Mamie", "mets tf1");
    assert!(rig.deps.store.inbox_recent(5).unwrap().is_empty());
    let rig = rig_with_model();
    rig.agent.set_paused(true);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    assert!(rig.deps.store.inbox_recent(5).unwrap().is_empty());
    rig.agent.set_paused(false);
    rig.hear("m-2", "Mamie", "mets tf1");
    assert_eq!(rig.deps.store.inbox_recent(5).unwrap().len(), 1);
}

#[test]
fn a_message_older_than_three_minutes_is_never_answered() {
    let rig = rig_with_model();
    let mut old = rig.chat("m-old", "Mamie", "mets tf1");
    old["at"] = json!(rig.now() - 200_000);
    rig.agent.on_event("party.message", &old);
    let row = rig.deps.store.inbox_recent(5).unwrap().remove(0);
    assert_eq!(row.reason.as_deref(), Some("stale"));
}
