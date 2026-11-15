
use super::support::{at, direct, none, programme, score, synth};
use crate::detect::{confirms, contradicts, decide, Verdict};

#[test]
fn ligue_1_sigils_without_a_guide_do_not_contradict() {
    let now = at("20:00", 3);
    let monaco = synth("m", "Monaco", "Rennes", ("MON", "REN"), "", now - 600.0);

    for (a, b) in [("ASM", "SRFC"), ("SRFC", "ASM")] {
        let reading = direct(|r| {
            r.sigil1 = a.into();
            r.sigil2 = b.into();
        });
        assert!(
            !contradicts(&monaco, Some(&reading), &[]) && confirms(&monaco, Some(&reading), &[])
        );
    }
    for (home, away, abbreviations, sigils) in [
        ("Toulouse", "Metz", ("TOU", "MET"), ("TFC", "FCM")),
        ("Auxerre", "Angers", ("AUX", "ANG"), ("AJA", "SCO")),
        ("Nantes", "Paris FC", ("NAN", "PFC"), ("FCN", "PFC")),
    ] {
        let m = synth("x", home, away, abbreviations, "", now - 600.0);
        let reading = direct(|r| {
            r.sigil1 = sigils.0.into();
            r.sigil2 = sigils.1.into();
        });
        assert_eq!(
            decide(std::slice::from_ref(&m), &[], now, Some(&reading), &none()).verdict,
            Verdict::Ok,
            "{home}"
        );
        assert!(!contradicts(&m, Some(&reading), &[]));
    }
}

#[test]
fn an_unknown_or_ambiguous_sigil_is_neutral_never_another_match() {
    let now = at("20:00", 3);
    let champions = "UEFA Champions League";
    let united = synth(
        "mu",
        "Manchester United",
        "AS Roma",
        ("MAN", "ROMA"),
        champions,
        now - 600.0,
    );
    let bayern = synth(
        "bm",
        "Atlético Madrid",
        "Bayern Munich",
        ("ATM", "MUN"),
        champions,
        now - 600.0,
    );
    let banner = direct(|r| {
        r.sigil1 = "MUN".into();
        r.sigil2 = "ROM".into();
    });
    let unknown = direct(|r| {
        r.sigil1 = "XYZ".into();
        r.sigil2 = "QWE".into();
    });
    let copied = direct(|r| {
        r.team1 = "MUN".into();
        r.team2 = "ROM".into();
        r.sigil1 = "MUN".into();
        r.sigil2 = "ROM".into();
    });
    let named_other = direct(|r| {
        r.team1 = "Lens".into();
        r.team2 = "Lille".into();
    });

    assert!(!contradicts(&united, Some(&banner), &[]));
    assert!(!contradicts(
        &united,
        Some(&banner),
        &[united.clone(), bayern]
    ));
    assert!(!contradicts(&united, Some(&unknown), &[]));
    assert!(contradicts(&united, Some(&named_other), &[]));
    assert!(!contradicts(&united, Some(&copied), &[]));
}

#[test]
fn a_sigil_naming_two_candidate_matches_names_neither() {
    let now = at("20:00", 3);
    let inter = synth("a", "Inter", "Torino", ("INT", "TOR"), "", now - 600.0);
    let internacional = synth(
        "b",
        "Internacional",
        "Gremio",
        ("INT", "GRE"),
        "",
        now - 600.0,
    );
    let reading = direct(|r| r.sigil1 = "INT".into());

    assert_eq!(
        score(&inter, now, Some(&reading), &[inter.clone(), internacional]),
        0.0
    );
    assert_eq!(
        score(&inter, now, Some(&reading), std::slice::from_ref(&inter)),
        5.0
    );
}

#[test]
fn a_generic_guide_and_a_colliding_sigil_do_not_retain_the_other_match() {
    let now = at("20:07", 3);
    let champions = "UEFA Champions League";
    let united = synth(
        "mu",
        "Manchester United",
        "AS Roma",
        ("MAN", "ROMA"),
        champions,
        now - 420.0,
    );
    let bayern = synth(
        "bm",
        "Atlético Madrid",
        "Bayern Munich",
        ("ATM", "MUN"),
        champions,
        now - 420.0,
    );
    let guide = [programme(now - 600.0, now + 6000.0, champions, "")];
    let banner = direct(|r| {
        r.sigil1 = "MUN".into();
        r.sigil2 = "ROM".into();
    });

    let decision = decide(&[united, bayern], &guide, now, Some(&banner), &none());

    assert!(
        decision.verdict != Verdict::Ok
            || decision
                .matched
                .is_some_and(|m| m.label() == "Manchester United – AS Roma")
    );
}
