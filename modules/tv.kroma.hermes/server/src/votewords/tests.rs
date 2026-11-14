use super::parse_vote;

#[test]
fn negations_and_refusals_are_never_read_as_a_vote_for() {
    let cases: &[(&str, Option<bool>)] = &[
        ("oui mais non", Some(false)),
        ("oui non", Some(false)),
        ("moi non", Some(false)),
        ("not ok", Some(false)),
        ("pas \u{1F44D}", None),
        ("jamais \u{1F44D}", None),
        ("oui... non en fait", None),
        ("pas ok", Some(false)),
        ("oui je suis d'accord", Some(true)),
        ("\u{1F44D}", Some(true)),
        ("\u{1F44E}", Some(false)),
        ("non merci", Some(false)),
        ("ok go", Some(true)),
        ("go toutes les tester", None),
        ("normal, on reste", None),
        ("pas de souci", None),
        ("c'est pas go", Some(false)),
        ("surtout pas", Some(false)),
        ("pas d\u{2019}accord", Some(false)),
        ("", None),
    ];
    let wrong: Vec<_> = cases
        .iter()
        .filter(|(text, want)| parse_vote(text) != *want)
        .map(|(text, want)| (*text, *want, parse_vote(text)))
        .collect();
    assert!(wrong.is_empty(), "{wrong:?}");
}
