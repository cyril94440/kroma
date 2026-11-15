
use super::support::{at, guide, label, matches_at, named, none, programme};
use crate::detect::{decide, Verdict};

const VILLARREAL: &str = "Villarreal – Paris Saint-Germain";

#[test]
fn villarreal_psg_is_detected_from_the_canal_foot_guide_with_readable_reasons() {
    let now = at("20:07", 3);

    let decision = decide(
        &matches_at(now),
        &guide("canalplusfoot.fr"),
        now,
        None,
        &none(),
    );

    assert_eq!(
        (decision.verdict, label(&decision).as_deref()),
        (Verdict::Ok, Some(VILLARREAL))
    );
    assert!(decision.reasons.iter().any(|r| r.contains("deux équipes")));
    assert_eq!(
        decision
            .matched
            .as_ref()
            .map(|m| (m.id.as_str(), m.venue.as_str())),
        Some(("espn-401915309", "Estadio de la Cerámica"))
    );
}

#[test]
fn the_generic_title_and_the_stadium_alone_still_find_the_match() {
    let now = at("20:07", 3);
    let only_generic: Vec<_> = guide("canalplusfoot.fr")
        .into_iter()
        .filter(|p| p.title != "Villarreal / Paris-SG")
        .collect();

    let decision = decide(&matches_at(now), &only_generic, now, None, &none());

    assert_eq!(label(&decision).as_deref(), Some(VILLARREAL));
    assert!(decision
        .reasons
        .contains(&"stade cité par le guide".to_string()));
    assert!(decision
        .reasons
        .contains(&"compétition citée par le guide".to_string()));
}

#[test]
fn a_champions_league_title_without_teams_is_ambiguous() {
    let now = at("20:07", 3);
    let generic = [programme(
        at("19:55", 3),
        at("22:05", 3),
        "UEFA Champions League",
        "",
    )];

    let decision = decide(&matches_at(now), &generic, now, None, &none());
    let empty = decide(&matches_at(now), &[], now, None, &none());

    assert_eq!(
        (decision.verdict, decision.matched.is_none()),
        (Verdict::Ambiguous, true)
    );
    assert_eq!(decision.candidates.len(), 7);
    assert!(decision
        .candidates
        .iter()
        .all(|c| c.competition == "UEFA Champions League"));
    assert_eq!(empty.verdict, Verdict::Nothing);
}

#[test]
fn barcelona_and_bodo_glimt_titles_find_their_matches() {
    let now = at("20:07", 3);
    let late = at("23:30", 3);
    let half = at("20:30", 3);
    let window = |title: &str| [programme(at("19:55", 3), at("22:05", 3), title, "")];

    let barca = decide(
        &matches_at(now),
        &window("FC Barcelone / Aston Villa"),
        now,
        None,
        &none(),
    );
    let finished: Vec<_> = guide("canalplusfoot.fr")
        .into_iter()
        .filter(|p| ["Bodø/Glimt / Lille", "UEFA Champions League"].contains(&p.title.as_str()))
        .collect();
    let too_late = decide(&matches_at(late), &finished, late, None, &none());
    let bodo = decide(
        &matches_at(half),
        &window("Bodø/Glimt / Lille"),
        half,
        None,
        &none(),
    );

    assert_eq!(label(&barca).as_deref(), Some("Barcelona – Aston Villa"));
    assert_eq!(too_late.verdict, Verdict::Nothing);
    assert_eq!(label(&bodo).as_deref(), Some("Bodo/Glimt – Lille"));
}

#[test]
fn a_magazine_citing_another_match_in_passing_is_not_retained() {
    let now = at("20:30", 3);
    let magazine = [programme(
        at("19:55", 3),
        at("22:05", 3),
        "En route pour Madrid",
        "en route pour madrid le paris-sg au fc barcelone ce soir",
    )];

    let decision = decide(&matches_at(now), &magazine, now, None, &none());

    assert_ne!(decision.verdict, Verdict::Ok);
}

#[test]
fn a_multiplex_naming_two_matches_is_ambiguous() {
    let now = at("20:07", 3);
    let text = "multiplex place a la 4e journee de ligue des champions, avec notamment le paris-sg, qui se deplace sur le terrain de villarreal, et lille qui se rend en norvege pour affronter bodo glimt.";

    let decision = decide(
        &matches_at(now),
        &[programme(at("19:55", 3), at("22:05", 3), "Multiplex", text)],
        now,
        None,
        &none(),
    );

    assert_eq!(decision.verdict, Verdict::Ambiguous);
    let labels: Vec<&str> = decision
        .candidates
        .iter()
        .map(|c| c.label.as_str())
        .collect();
    assert!(labels.contains(&VILLARREAL) && labels.contains(&"Bodo/Glimt – Lille"));
}

#[test]
fn the_night_rerun_gives_nothing() {
    let now = at("00:03", 4);
    let matches = matches_at(now);

    let decision = decide(&matches, &guide("canalplusfoot.fr"), now, None, &none());

    assert_eq!(named(&matches, "Villarreal").state, "post");
    assert_eq!(decision.verdict, Verdict::Nothing);
}

#[test]
fn the_pre_match_show_retains_the_match_in_pre_and_too_early_is_not_a_candidate() {
    let now = at("19:30", 3);
    let early = at("19:10", 3);

    let decision = decide(
        &matches_at(now),
        &guide("canalplusfoot.fr"),
        now,
        None,
        &none(),
    );
    let too_early = decide(
        &matches_at(early),
        &guide("canalplusfoot.fr"),
        early,
        None,
        &none(),
    );

    assert_eq!(
        (
            decision.verdict,
            label(&decision).as_deref(),
            decision.matched.as_ref().map(|m| m.state.as_str())
        ),
        (Verdict::Ok, Some(VILLARREAL), Some("pre"))
    );
    assert!(decision
        .reasons
        .iter()
        .filter(|r| r.contains("guide"))
        .all(|r| r.contains("programme suivant")));
    assert_eq!(too_early.verdict, Verdict::Nothing);
}

#[test]
fn other_channels_show_no_champions_league_match_and_the_multi_match_channels_one_each() {
    let now = at("20:07", 3);
    let on = |channel: &str| {
        label(&decide(
            &matches_at(now),
            &guide(channel),
            now,
            None,
            &none(),
        ))
    };

    for channel in [
        "beinsports1.fr",
        "lequipe21.fr",
        "rmcsport1.fr",
        "canalplus.fr",
    ] {
        let decision = decide(&matches_at(now), &guide(channel), now, None, &none());
        assert!(
            matches!(decision.verdict, Verdict::Nothing | Verdict::Ambiguous)
                && decision.matched.is_none(),
            "{channel}"
        );
    }
    assert_eq!(
        on("canalpluslive2.fr").as_deref(),
        Some("Bodo/Glimt – Lille")
    );
    assert_eq!(
        on("canalpluslive4.fr").as_deref(),
        Some("Barcelona – Aston Villa")
    );
    assert_eq!(
        on("canalpluslive5.fr").as_deref(),
        Some("Manchester United – AS Roma")
    );
}
