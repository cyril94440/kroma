use super::support::{at, direct, programme, synth};
use crate::detect::scoring::rate;
use crate::detect::{
    confirms, contradicts, decide, name_designates, team_forms, TeamNames, Verdict,
};
use crate::text::norm;

fn forms(display: &str, short: &str, abbreviation: &str) -> crate::detect::Forms {
    team_forms(
        &TeamNames {
            display: display.into(),
            short: short.into(),
            abbreviation: abbreviation.into(),
            ..TeamNames::default()
        },
        true,
    )
}

#[test]
fn normalisation_does_not_cut_bodo_glimt() {
    assert_eq!(norm("Bodø/Glimt"), "bodo glimt");
    assert!(team_forms(
        &TeamNames {
            display: "Bodo/Glimt".into(),
            ..TeamNames::default()
        },
        false
    )
    .contains("bodo glimt"));
}

#[test]
fn competitions_are_read_in_french_and_english() {
    use crate::detect::{slug_competition, slug_espn};

    assert_eq!(
        slug_competition("UEFA Champions League"),
        Some("uefa.champions")
    );
    assert_eq!(
        slug_competition("Ligue des champions"),
        Some("uefa.champions")
    );
    assert_eq!(slug_competition("Ligue 1 Uber Eats"), Some("fra.1"));
    assert_eq!(slug_competition("AFC Champions League Elite"), None);
    assert_eq!(
        slug_espn("UEFA Champions League, League Phase"),
        Some("uefa.champions")
    );
    assert_eq!(slug_espn("Brazil Serie A"), None);
}

#[test]
fn a_generic_name_does_not_designate_two_teams() {
    let now = at("20:00", 3);
    let inter = synth(
        "i",
        "Inter Milan",
        "Torino",
        ("INT", "TOR"),
        "",
        now - 600.0,
    );
    let milan = synth("m", "AC Milan", "Napoli", ("MIL", "NAP"), "", now - 600.0);
    let pool = [inter.clone(), milan.clone()];
    let screen = direct(|r| {
        r.team1 = "AC Milan".into();
        r.team2 = "Naples".into();
    });
    let single = |name: &str| direct(|r| r.team1 = name.into());
    let points = |m: &crate::detect::FlatMatch, image: &crate::sport::Reading| {
        rate(m, &[], now, Some(image), &pool).0
    };
    let (milan_word, inter_word) = (single("Milan"), single("Inter"));

    assert!(contradicts(&inter, Some(&screen), &pool));
    assert!(confirms(&milan, Some(&screen), &pool) && !contradicts(&milan, Some(&screen), &pool));
    assert!(points(&milan, &screen) > points(&inter, &screen));
    assert_eq!(points(&inter, &milan_word), 0.0);
    assert_eq!(points(&milan, &inter_word), 0.0);
    assert_eq!(points(&milan, &milan_word), 5.0);
    assert_eq!(points(&inter, &inter_word), 5.0);
}

#[test]
fn madrid_and_manchester_are_ambiguous_when_both_clubs_are_candidates() {
    let now = at("20:00", 3);
    let real = synth("r", "Real Madrid", "Lyon", ("", ""), "", now - 600.0);
    let atletico = synth("a", "Atlético Madrid", "Lille", ("", ""), "", now - 600.0);
    let reading = |name: &str| direct(|r| r.team1 = name.into());
    let both = [real.clone(), atletico];

    assert_eq!(
        rate(&real, &[], now, Some(&reading("Madrid")), &both).0,
        0.0
    );
    assert_eq!(
        rate(
            &real,
            &[],
            now,
            Some(&reading("Madrid")),
            std::slice::from_ref(&real)
        )
        .0,
        5.0
    );
    assert_eq!(
        rate(&real, &[], now, Some(&reading("Real Madrid")), &both).0,
        5.0
    );
}

#[test]
fn an_exact_form_shared_by_two_teams_is_neutral() {
    let bayern = forms("Bayern Munich", "", "MUN");
    let united = forms("Manchester United", "", "MAN");

    assert!(name_designates("MUN", &bayern, &[]) && name_designates("MUN", &united, &[]));
    assert!(
        !name_designates("MUN", &bayern, std::slice::from_ref(&united))
            && !name_designates("MUN", &united, std::slice::from_ref(&bayern))
    );
    assert!(name_designates(
        "Bayern Munich",
        &bayern,
        std::slice::from_ref(&united)
    ));
}

#[test]
fn rangers_as_a_whole_name_stays_a_form() {
    let now = at("19:00", 3);
    let rangers = synth(
        "r",
        "Rangers",
        "Fenerbahce",
        ("RAN", "FEN"),
        "UEFA Europa League",
        now - 300.0,
    );
    let guide = [programme(
        now - 600.0,
        now + 6000.0,
        "Rangers / Fenerbahce",
        "rangers fenerbahce europa league",
    )];

    assert!(forms("Rangers", "Rangers", "").contains("rangers"));
    assert!(!forms("Queens Park Rangers", "QPR", "").contains("rangers"));
    assert!(!forms("Paris FC", "Paris FC", "").contains("paris"));
    assert_eq!(
        decide(&[rangers], &guide, now, None, &Default::default()).verdict,
        Verdict::Ok
    );
    assert!(name_designates("Rangers", &forms("Rangers", "", ""), &[]));
}
