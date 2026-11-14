use super::*;

fn people(names: &[&str]) -> Vec<Person> {
    names.iter().map(|n| Person::new(&n.to_lowercase(), n)).collect()
}

fn channel() -> VoteKind {
    VoteKind::Channel {
        channel_id: "tf1".into(),
        channel_name: "TF1".into(),
    }
}

fn open(requester: Option<&str>, present: &[&str]) -> Vote {
    let who = requester.map(|n| Person::new(&n.to_lowercase(), n));
    Vote::open(Opening {
        id: 1,
        kind: channel(),
        label: "TF1",
        note: "",
        requester: who,
        present: &people(present),
        now: 0,
        seconds: 30,
    })
}

fn cast(vote: &mut Vote, name: &str, approve: bool, present: &[&str]) -> Recorded {
    let p = people(present);
    vote.record(&Person::new(&name.to_lowercase(), name), approve, Some(&p))
}

#[test]
fn the_vote_closes_as_soon_as_the_majority_is_certain() {
    let all = ["Cyril", "Mamie", "Papi", "Ced", "Lea"];
    let mut v = open(Some("Cyril"), &all);
    assert_eq!(v.required.len(), 4);
    assert_eq!(cast(&mut v, "Mamie", true, &all), Recorded::Counted);
    assert_eq!(cast(&mut v, "Papi", true, &all), Recorded::Decided);
    assert_eq!(v.verdict(), Verdict::Passed);

    let all = ["Cyril", "Mamie", "Papi", "Ced"];
    let mut v = open(Some("Cyril"), &all);
    assert_eq!(cast(&mut v, "Mamie", false, &all), Recorded::Counted);
    assert_eq!(cast(&mut v, "Papi", false, &all), Recorded::Decided);
    assert_eq!(v.verdict(), Verdict::Refused);
}

#[test]
fn the_request_counts_for_and_a_blank_counts_for_nothing() {
    let all = ["Benji", "Ced"];
    let v = open(Some("Benji"), &all);
    assert_eq!(v.tally(), Tally { yes: 1, no: 0, blank: 1 });
    assert_eq!(v.verdict(), Verdict::Passed);

    let mut v = open(Some("Benji"), &all);
    cast(&mut v, "Ced", false, &all);
    assert_eq!(v.tally(), Tally { yes: 1, no: 1, blank: 0 });
    assert_eq!(v.verdict(), Verdict::Refused);
}

#[test]
fn a_proposal_of_the_agent_has_no_voice_of_its_own() {
    let v = open(None, &["Benji", "Ced"]);
    assert_eq!(v.tally(), Tally { yes: 0, no: 0, blank: 2 });
    assert_eq!(v.verdict(), Verdict::Refused);
    assert_eq!(v.required.len(), 2);
}

#[test]
fn one_person_is_one_voice_and_the_requester_never_votes() {
    let all = ["Ced", "Cyril", "Mamie"];
    let mut v = open(Some("Cyril"), &all);
    assert_eq!(v.required.keys().cloned().collect::<Vec<_>>(), vec!["ced", "mamie"]);
    assert!(!v.accepts_voter("cyril"));
    assert_eq!(cast(&mut v, "Cyril", true, &all), Recorded::Ignored);
    assert!(v.votes.is_empty());
    cast(&mut v, "Ced", false, &all);
    cast(&mut v, "Ced", true, &all);
    assert_eq!(v.votes.get("ced"), Some(&true));
    assert_eq!(v.votes.len(), 1);
}

#[test]
fn a_late_arrival_can_vote_but_a_stranger_cannot() {
    let mut v = open(Some("Cyril"), &["Cyril", "Ced", "Mamie"]);
    let now = ["Cyril", "Ced", "Mamie", "Sebi"];
    assert_eq!(cast(&mut v, "Sebi", true, &now), Recorded::Counted);
    assert!(v.is_required("sebi"));
    assert_eq!(cast(&mut v, "Intrus", true, &now), Recorded::Ignored);
    assert!(!v.is_required("intrus"));
    let view = v.view(0);
    assert_eq!(view.tally, Tally { yes: 2, no: 0, blank: 2 });
}

#[test]
fn an_unknown_presence_never_admits_a_late_voter() {
    let mut v = open(Some("Cyril"), &["Cyril", "Ced"]);
    let got = v.record(&Person::new("sebi", "Sebi"), true, None);
    assert_eq!(got, Recorded::Ignored);
}

#[test]
fn a_device_leaving_does_not_remove_the_person_while_the_account_is_still_present() {
    let mut v = open(Some("Cyril"), &["Cyril", "Mamie", "Papi"]);
    assert_eq!(v.reevaluate(&people(&["Cyril", "Mamie", "Papi"]), 1000, 45_000), Reevaluated::Unchanged);
    assert_eq!(v.required.len(), 2);
}

#[test]
fn a_voter_gone_longer_than_the_grace_leaves_the_quorum() {
    let mut v = open(Some("Cyril"), &["Cyril", "Mamie", "Papi"]);
    let here = people(&["Cyril", "Papi"]);
    assert_eq!(v.reevaluate(&here, 1_000, 45_000), Reevaluated::Unchanged);
    assert_eq!(v.required.len(), 2);
    assert_eq!(v.reevaluate(&here, 47_000, 45_000), Reevaluated::Updated);
    assert_eq!(v.required.keys().cloned().collect::<Vec<_>>(), vec!["papi"]);
}

#[test]
fn a_flickering_connection_stays_in_the_vote() {
    let mut v = open(Some("Benji"), &["Benji", "Ced"]);
    let alone = people(&["Benji"]);
    let both = people(&["Benji", "Ced"]);
    v.reevaluate(&alone, 1000, 45_000);
    v.reevaluate(&alone, 20_000, 45_000);
    assert!(v.is_required("ced"));
    v.reevaluate(&both, 30_000, 45_000);
    v.reevaluate(&alone, 40_000, 45_000);
    v.reevaluate(&alone, 80_000, 45_000);
    assert!(v.is_required("ced"));
    assert_eq!(v.reevaluate(&alone, 90_000, 45_000), Reevaluated::Unchanged);
    assert_eq!(v.reevaluate(&alone, 110_000, 45_000), Reevaluated::Decided);
    assert_eq!(v.verdict(), Verdict::Alone);
}

#[test]
fn the_vote_is_cancelled_when_the_requester_and_the_voters_are_gone() {
    let mut v = open(Some("Cyril"), &["Cyril", "Mamie"]);
    assert_eq!(v.reevaluate(&[], 1000, 0), Reevaluated::Unchanged);
    assert_eq!(v.reevaluate(&[], 2000, 0), Reevaluated::Cancelled);
}

#[test]
fn a_single_empty_reading_keeps_the_quorum_and_a_return_resets_the_count() {
    let mut v = open(Some("Cyril"), &["Cyril", "Mamie", "Papi"]);
    assert_eq!(v.reevaluate(&[], 1000, 45_000), Reevaluated::Unchanged);
    assert_eq!(v.required.len(), 2);
    assert_eq!(v.reevaluate(&people(&["Cyril", "Papi"]), 2000, 45_000), Reevaluated::Unchanged);
    assert_eq!(v.reevaluate(&people(&["Cyril"]), 3000, 45_000), Reevaluated::Unchanged);
}

#[test]
fn the_requester_alone_after_the_voters_left_switches_alone() {
    let mut v = open(Some("Cyril"), &["Cyril", "Mamie"]);
    let alone = people(&["Cyril"]);
    assert_eq!(v.reevaluate(&alone, 1000, 0), Reevaluated::Unchanged);
    assert_eq!(v.reevaluate(&alone, 2000, 0), Reevaluated::Decided);
    assert_eq!(v.verdict(), Verdict::Alone);
}

#[test]
fn no_quorum_when_nobody_else_is_there() {
    let p = people(&["Cyril"]);
    assert!(!Vote::has_quorum(&p, Some(&Person::new("cyril", "Cyril"))));
    assert!(Vote::has_quorum(&p, None));
    assert!(Vote::has_quorum(&people(&["Cyril", "Ced"]), Some(&Person::new("cyril", "Cyril"))));
}

#[test]
fn the_view_pins_the_json_the_clients_read() {
    let mut v = open(Some("Cyril"), &["Cyril", "Ced"]);
    cast(&mut v, "Ced", true, &["Cyril", "Ced"]);
    let json = serde_json::to_value(v.view(10_000)).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "id": 1,
            "kind": "channel",
            "label": "TF1",
            "note": "",
            "by": { "userId": "cyril", "name": "Cyril" },
            "endsAt": 30000,
            "endsInS": 20,
            "required": [{ "userId": "ced", "name": "Ced" }],
            "votes": { "ced": true },
            "tally": { "yes": 2, "no": 0, "blank": 0 },
            "channelId": "tf1"
        })
    );
}
