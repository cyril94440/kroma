
use super::support::{at, direct, matches_at, named, none, programme, score, synth};
use crate::detect::{confirms, contradicts, decide, score_compatible, Verdict};
use crate::sport::Reading;

fn generic_guide() -> [crate::detect::DetectProgramme; 1] {
    [programme(
        at("19:55", 3),
        at("22:05", 3),
        "UEFA Champions League",
        "",
    )]
}

#[test]
fn an_image_reading_names_the_match_with_or_without_guide() {
    let now = at("20:07", 3);
    let sigils = direct(|r| {
        r.sigil1 = "VIL".into();
        r.sigil2 = "PSG".into();
        r.score = "0-0".into();
    });
    let short = direct(|r| {
        r.team1 = "Villarreal".into();
        r.team2 = "PSG".into();
    });
    let full = direct(|r| {
        r.team1 = "Villarreal CF".into();
        r.team2 = "Paris Saint-Germain".into();
        r.competition = "Ligue des champions".into();
    });

    for reading in [&sigils, &short, &full] {
        let with_guide = decide(
            &matches_at(now),
            &generic_guide(),
            now,
            Some(reading),
            &none(),
        );
        let without = decide(&matches_at(now), &[], now, Some(reading), &none());
        assert_eq!(
            with_guide.matched.map(|m| m.label()).as_deref(),
            Some("Villarreal – Paris Saint-Germain")
        );
        assert_eq!(
            without.matched.map(|m| m.label()).as_deref(),
            Some("Villarreal – Paris Saint-Germain")
        );
    }
}

#[test]
fn a_single_team_is_not_enough_without_guide_and_unsure_or_ambiguous_readings_count_for_nothing() {
    let now = at("20:07", 3);
    let one = direct(|r| r.team1 = "PSG".into());
    let unsure = direct(|r| {
        r.team1 = "Villarreal".into();
        r.team2 = "PSG".into();
        r.confidence = 0.3;
    });
    let bare_paris = direct(|r| r.team1 = "Paris".into());

    assert_eq!(
        decide(&matches_at(now), &[], now, Some(&one), &none()).verdict,
        Verdict::Ambiguous
    );
    assert_eq!(
        decide(&matches_at(now), &generic_guide(), now, Some(&one), &none()).verdict,
        Verdict::Ok
    );
    assert_eq!(
        decide(&matches_at(now), &[], now, Some(&unsure), &none()).verdict,
        Verdict::Nothing
    );
    assert_eq!(
        decide(&matches_at(now), &[], now, Some(&bare_paris), &none()).verdict,
        Verdict::Nothing
    );
}

#[test]
fn the_read_score_must_not_exceed_espns() {
    let mut m = synth("v", "Villarreal", "PSG", ("", ""), "", 0.0);
    m.score = "1-0".into();

    assert_eq!(score_compatible("0-0", &m), Some(true));
    assert_eq!(score_compatible("1-0", &m), Some(true));
    assert_eq!(score_compatible("0-3", &m), Some(false));
    assert_eq!(score_compatible("", &m), None);
}

#[test]
fn the_image_contradicts_a_wrong_match_and_confirms_the_right_one() {
    let now = at("20:07", 3);
    let matches = matches_at(now);
    let villarreal = named(&matches, "Villarreal");
    let lens_lille = direct(|r| {
        r.team1 = "Lens".into();
        r.team2 = "Lille".into();
    });

    assert!(contradicts(villarreal, Some(&lens_lille), &[]));
    assert!(!contradicts(
        villarreal,
        Some(&Reading {
            confidence: 0.3,
            ..lens_lille.clone()
        }),
        &[]
    ));
    assert!(!contradicts(
        villarreal,
        Some(&direct(|r| {
            r.team1 = "Villarreal".into();
            r.team2 = "PSG".into();
        })),
        &[]
    ));
    assert!(confirms(
        villarreal,
        Some(&direct(|r| r.team1 = "Villarreal".into())),
        &[]
    ));
    assert!(!contradicts(villarreal, Some(&direct(|_| {})), &[]));
}

#[test]
fn a_reading_that_is_not_live_is_neutral() {
    let now = at("20:52", 3);
    let matches = matches_at(at("20:07", 3));
    let villarreal = named(&matches, "Villarreal");
    let other = |kind: &str| Reading {
        kind: kind.into(),
        team1: "Barcelone".into(),
        team2: "Aston Villa".into(),
        sigil1: "BAR".into(),
        sigil2: "AVL".into(),
        confidence: 0.85,
        ..Reading::default()
    };
    let baseline = score(villarreal, now, None, &[]);

    assert!(contradicts(villarreal, Some(&other("direct")), &[]));
    for kind in ["resume", "plateau", "pub", "mire", "autre", "inconnu", ""] {
        let reading = other(kind);
        assert!(
            !contradicts(villarreal, Some(&reading), &[])
                && !confirms(villarreal, Some(&reading), &[]),
            "{kind}"
        );
        assert_eq!(
            score(villarreal, now, Some(&reading), &[]),
            baseline,
            "{kind}"
        );
    }
    let right = |kind: &str| Reading {
        kind: kind.into(),
        team1: "Villarreal".into(),
        team2: "PSG".into(),
        confidence: 0.9,
        ..Reading::default()
    };
    assert_eq!(score(villarreal, now, Some(&right("resume")), &[]), 0.0);
    assert!(score(villarreal, now, Some(&right("direct")), &[]) >= 12.0);
    assert_eq!(
        decide(
            &matches_at(at("20:07", 3)),
            &[],
            at("20:07", 3),
            Some(&other("resume")),
            &none()
        )
        .verdict,
        Verdict::Nothing
    );
}
