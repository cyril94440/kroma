use super::*;

fn who(name: &str) -> Person {
    Person::new(&name.to_lowercase(), name)
}

fn yesno() -> Poll {
    Poll::start(1, "Match ?", "yesno", &[], None, 0).unwrap()
}

#[test]
fn a_yes_no_poll_accepts_words_and_thumbs_and_nothing_else() {
    let mut p = yesno();
    assert!(p.vote(&who("Mamie"), " OUI "));
    assert!(p.vote(&who("Papi"), "\u{1F44E}"));
    assert!(!p.vote(&who("Seb"), "peut-\u{ea}tre"));
    assert_eq!(p.votes["mamie"].value, "Oui");
    assert_eq!(p.votes["papi"].value, "Non");
    assert_eq!(p.votes.len(), 2);
}

#[test]
fn everyone_present_having_voted_ends_the_poll_but_never_a_solo_viewer() {
    let mut p = yesno();
    let room = [who("Mamie"), who("Papi")];
    p.vote(&room[0], "oui");
    assert!(!p.everyone_voted(&room));
    p.vote(&room[1], "non");
    assert!(p.everyone_voted(&room));
    assert!(!p.everyone_voted(&room[..1]));
}

#[test]
fn a_person_changing_their_mind_replaces_their_vote() {
    let mut p = yesno();
    p.vote(&who("Cyril"), "oui");
    p.vote(&who("Cyril"), "non");
    let view = p.view(0);
    assert_eq!(view.total, 1);
    assert_eq!(view.votes["cyril"], "Non");
}

#[test]
fn a_long_option_is_clipped_alike_at_creation_and_at_vote() {
    let long = "x".repeat(80);
    let mut p = Poll::start(1, "Qui ?", "choice", &[long.clone(), "Lens".into()], None, 0).unwrap();
    assert_eq!(p.options[0].chars().count(), OPTION_MAX);
    assert!(p.vote(&who("A"), &long));
    assert!(p.vote(&who("B"), &long));
    assert_eq!(p.view(0).total, 2);
}

#[test]
fn a_choice_only_counts_an_existing_option_and_accents_fold() {
    let mut p = Poll::start(1, "Qui ?", "choice", &["Lens".into(), "Paris".into(), "Nîmes".into()], None, 0).unwrap();
    assert!(p.vote(&who("A"), "lens"));
    assert!(!p.vote(&who("B"), "Nice"));
    assert!(p.vote(&who("C"), "nimes"));
    assert_eq!(p.votes["c"].value, "Nîmes");
}

#[test]
fn a_score_poll_takes_a_score_and_rejects_anything_else() {
    let mut p = Poll::start(1, "Score ?", "score", &[], None, 0).unwrap();
    assert!(p.vote(&who("A"), "2 : 1"));
    assert_eq!(p.votes["a"].value, "2-1");
    assert!(!p.vote(&who("B"), "__proto__"));
    assert!(!p.vote(&who("B"), "123-1"));
    assert_eq!(p.view(0).total, 1);
}

#[test]
fn an_open_poll_groups_equivalent_answers() {
    let mut p = Poll::start(1, "Qui gagne ?", "open", &[], None, 0).unwrap();
    assert!(p.vote(&who("A"), "Marseille"));
    assert!(p.vote(&who("B"), "marseille"));
    assert!(!p.vote(&who("C"), &"y".repeat(41)));
    assert_eq!(p.view(0).by_answer["Marseille"], vec!["A", "B"]);
}

#[test]
fn invalid_polls_are_refused_with_the_source_codes() {
    assert_eq!(Poll::start(1, "", "yesno", &[], None, 0).unwrap_err(), PollError::InvalidQuestionOrType);
    assert_eq!(Poll::start(1, "x", "weird", &[], None, 0).unwrap_err(), PollError::InvalidQuestionOrType);
    assert_eq!(
        Poll::start(1, "x", "choice", &["seul".into()], None, 0).unwrap_err(),
        PollError::ChoiceNeedsTwoOptions
    );
}

#[test]
fn the_duration_is_clamped_between_twenty_seconds_and_fifteen_minutes() {
    assert_eq!(Poll::start(1, "x", "yesno", &[], Some(5), 0).unwrap().view(0).ends_in_s, 20);
    assert_eq!(Poll::start(1, "x", "yesno", &[], Some(9999), 0).unwrap().view(0).ends_in_s, 900);
    assert_eq!(Poll::start(1, "x", "yesno", &[], None, 0).unwrap().view(0).ends_in_s, 120);
}

#[test]
fn a_typed_answer_is_recognised_strictly() {
    let p = yesno();
    assert!(p.answer_from_chat("Oui").is_some());
    assert!(p.answer_from_chat("oui mais non").is_none());
    let s = Poll::start(1, "x", "score", &[], None, 0).unwrap();
    assert_eq!(s.answer_from_chat("2-1").as_deref(), Some("2-1"));
    assert!(s.answer_from_chat("bonne chance").is_none());
    let c = Poll::start(1, "x", "choice", &["A".into(), "B".into()], None, 0).unwrap();
    assert!(c.answer_from_chat("A").is_some());
}

#[test]
fn the_summary_lists_the_answers_by_popularity() {
    let mut p = yesno();
    p.vote(&who("Mamie"), "oui");
    p.vote(&who("Papi"), "oui");
    p.vote(&who("Ced"), "non");
    assert_eq!(Poll::summary(&p.view(0)), "Oui : 2 \u{2014} Mamie, Papi \u{b7} Non : 1 \u{2014} Ced");
}

#[test]
fn the_view_pins_the_json_the_clients_read() {
    let mut p = yesno();
    p.vote(&who("Mamie"), "oui");
    assert_eq!(
        serde_json::to_value(p.view(10_000)).unwrap(),
        serde_json::json!({
            "id": 1,
            "question": "Match ?",
            "kind": "yesno",
            "options": ["Oui", "Non"],
            "votes": { "mamie": "Oui" },
            "byAnswer": { "Non": [], "Oui": ["Mamie"] },
            "total": 1,
            "endsAt": 120000,
            "endsInS": 110
        })
    );
}
