use super::*;

fn reading(kind: &str, team1: &str, team2: &str, score: &str, clock: &str, confidence: f64) -> ScreenReading {
    let mut r = ScreenReading::unknown();
    r.kind = kind.into();
    r.team1 = team1.into();
    r.team2 = team2.into();
    r.score = score.into();
    r.clock = clock.into();
    r.confidence = confidence;
    r
}

fn reference() -> ScreenReading {
    let mut r = reading("direct", "RC Lens", "Paris FC", "1-0", "34:12", 0.9);
    r.tag1 = "RCL".into();
    r.tag2 = "PFC".into();
    r
}

fn verdict(candidate: &ScreenReading, dt: f64) -> Verdict {
    decide(&reference(), candidate, dt).0
}

#[test]
fn paris_fc_is_never_taken_for_psg_with_a_yes() {
    let psg = reading("direct", "Lens", "Paris Saint-Germain", "1-0", "34:20", 0.8);
    let mut tagged = psg.clone();
    tagged.tag2 = "PSG".into();
    let alone = reading("direct", "Lens", "Paris", "1-0", "34:20", 0.8);
    let alone_other_score = reading("direct", "Lens", "Paris", "2-0", "34:20", 0.8);

    assert_eq!(verdict(&psg, 0.0), Verdict::Probable);
    assert_eq!(verdict(&tagged, 0.0), Verdict::Probable);
    assert_eq!(verdict(&alone, 0.0), Verdict::Probable);
    assert_eq!(verdict(&alone_other_score, 0.0), Verdict::Unknown);
}

#[test]
fn the_psg_reference_does_not_confirm_paris_fc_either() {
    let mut psg = reference();
    psg.team2 = "Paris Saint-Germain".into();
    psg.tag2 = "PSG".into();
    let candidate = reading("direct", "RC Lens", "Paris FC", "1-0", "34:20", 0.9);

    assert_eq!(decide(&psg, &candidate, 0.0).0, Verdict::Probable);
}

#[test]
fn the_same_match_is_a_yes_across_providers_orders_and_delays() {
    let mut later = reference();
    later.clock = "34:52".into();
    let mut tags = ScreenReading::unknown();
    tags.kind = "direct".into();
    tags.tag1 = "RCL".into();
    tags.tag2 = "PFC".into();
    tags.score = "1 - 0".into();
    tags.confidence = 0.8;
    let swapped = reading("direct", "Paris FC", "Lens", "0-1", "34:12", 0.9);
    let no_score = reading("direct", "RC Lens", "Paris FC", "", "34:30", 0.9);
    let mut aged = reference();
    aged.clock = "35:40".into();

    assert_eq!(verdict(&later, 40.0), Verdict::Yes);
    assert_eq!(verdict(&tags, 0.0), Verdict::Yes);
    assert_eq!(verdict(&swapped, 0.0), Verdict::Yes);
    assert_eq!(verdict(&no_score, 10.0), Verdict::Yes);
    assert_eq!(verdict(&aged, 90.0), Verdict::Yes);
}

#[test]
fn another_programme_is_a_no() {
    let other_teams = reading("direct", "PSG", "Nantes", "1-0", "34:12", 0.9);
    let lyon = reading("direct", "Lyon", "Monaco", "0-0", "33:10", 0.9);
    let far_score = reading("direct", "RC Lens", "Paris FC", "3-0", "34:12", 0.9);

    assert_eq!(verdict(&other_teams, 0.0), Verdict::No);
    assert_eq!(verdict(&lyon, 0.0), Verdict::No);
    assert_eq!(verdict(&far_score, 0.0), Verdict::No);
    assert_eq!(verdict(&reading("noir", "", "", "", "", 1.0), 0.0), Verdict::No);
    assert_eq!(verdict(&reading("mire", "", "", "", "", 0.9), 0.0), Verdict::No);
}

#[test]
fn doubtful_readings_are_probable_or_unknown_never_yes() {
    let summary = reading("resume", "RC Lens", "Paris FC", "1-0", "", 0.9);
    let one_goal = reading("direct", "RC Lens", "Paris FC", "2-0", "35:30", 0.9);
    let replay = reading("direct", "RC Lens", "Paris FC", "1-0", "24:12", 0.9);
    let bare = reading("direct", "RC Lens", "Paris FC", "", "", 0.9);
    let unsure = reading("direct", "RC Lens", "Paris FC", "1-0", "34:12", 0.4);
    let nameless = reading("direct", "", "", "1-0", "34:12", 0.9);
    let mut weak_reference = reference();
    weak_reference.confidence = 0.3;

    assert_eq!(verdict(&summary, 0.0), Verdict::Probable);
    assert_eq!(verdict(&one_goal, 70.0), Verdict::Probable);
    assert_eq!(verdict(&replay, 0.0), Verdict::Probable);
    assert_eq!(verdict(&bare, 0.0), Verdict::Probable);
    assert_eq!(verdict(&unsure, 0.0), Verdict::Unknown);
    assert_eq!(verdict(&nameless, 0.0), Verdict::Unknown);
    assert_eq!(decide(&weak_reference, &replay, 0.0).0, Verdict::Unknown);
    assert_eq!(verdict(&ScreenReading::unknown(), 0.0), Verdict::Unknown);
}

#[test]
fn team_names_and_tags_fold_to_one_canonical_name() {
    assert_eq!(canon_team("Olympique Lyonnais"), Some("lyon".into()));
    assert_eq!(canon_team("OL"), Some("lyon".into()));
    assert_eq!(canon_team("Paris SG"), Some("psg".into()));
    assert_eq!(canon_team("Paris Saint-Germain"), Some("psg".into()));
    assert_eq!(canon_team("Paris FC"), canon_team("PFC"));
    assert_eq!(canon_team("Paris"), None);
    assert_eq!(canon_team(""), None);
    assert_eq!(canon_team("Stade Rennais"), Some("rennes".into()));
    assert_eq!(canon_team("AC Milan"), Some("milan".into()));
    assert_eq!(canon_team("Saint-Étienne"), canon_team("ASSE"));
}

#[test]
fn a_tag_supplies_an_unreadable_or_ambiguous_team_name() {
    let mut r = reading("direct", "Lens", "Paris", "", "", 0.9);
    r.tag2 = "PFC".into();

    assert_eq!(teams_of(&r), [Some("lens".into()), Some("paris fc".into())]);
    assert_eq!(teams_of(&reading("direct", "", "Paris", "", "", 0.9)), [None, None]);
}

#[test]
fn a_score_and_a_clock_are_parsed_in_every_displayed_form() {
    for text in ["1-0", "1 - 0", "1:0", "1–0"] {
        assert_eq!(parse_score(text), Some((1, 0)), "{text}");
    }
    assert_eq!(parse_score(""), None);
    assert_eq!(parse_clock("34:12"), Some(2052));
    assert_eq!(parse_clock("45+2"), Some(2820));
    assert_eq!(parse_clock("90'+4"), Some(5640));
    assert_eq!(parse_clock("67'"), Some(4020));
    assert_eq!(parse_clock("67’"), Some(4020));
    assert_eq!(parse_clock(""), None);
    assert_eq!(parse_clock("mi-temps"), None);
}

#[test]
fn a_searched_match_is_read_from_text_with_any_separator() {
    let cut = |text: &str| target_from_text(text);

    assert_eq!(cut("Lens - Paris FC"), (Some("Lens".into()), Some("Paris FC".into())));
    assert_eq!(cut("Lens contre PSG"), (Some("Lens".into()), Some("PSG".into())));
    assert_eq!(cut("Lens vs Paris FC"), (Some("Lens".into()), Some("Paris FC".into())));
    assert_eq!(cut("Lens"), (Some("Lens".into()), None));
}

#[test]
fn a_target_without_a_separator_is_cut_where_both_halves_are_known_teams() {
    let cut = |text: &str| target_from_text(text);

    assert_eq!(cut("Lille Lens"), (Some("Lille".into()), Some("Lens".into())));
    assert_eq!(cut("LOSC RC Lens"), (Some("LOSC".into()), Some("RC Lens".into())));
    assert_eq!(cut("Paris Saint Germain Lens"), (Some("Paris Saint Germain".into()), Some("Lens".into())));
    assert_eq!(cut("Le Havre PSG"), (Some("Le Havre".into()), Some("PSG".into())));
    assert_eq!(cut("Lille-Lens"), (Some("Lille".into()), Some("Lens".into())));
}

#[test]
fn a_candidate_is_judged_against_the_searched_match() {
    let target = target_from_text("Lens - Paris FC");
    let summary = reading("resume", "RC Lens", "Paris FC", "", "", 0.9);
    let one_team = target_from_text("Lens");
    let lyon = reading("direct", "Lyon", "Monaco", "", "", 0.9);
    let mut psg = reference();
    psg.team2 = "Paris Saint-Germain".into();
    psg.tag2 = "PSG".into();

    assert_eq!(match_target(&target, &reference()).0, Verdict::Yes);
    assert_eq!(match_target(&target, &summary).0, Verdict::Probable);
    assert_eq!(match_target(&one_team, &reference()).0, Verdict::Probable);
    assert_eq!(match_target(&target, &lyon).0, Verdict::No);
    assert_eq!(match_target(&target, &reading("noir", "", "", "", "", 1.0)).0, Verdict::No);
    assert_eq!(match_target(&target, &reading("direct", "RC Lens", "Paris FC", "", "", 0.2)).0, Verdict::Unknown);
    assert_eq!(match_target(&target, &psg).0, Verdict::Probable);
}

#[test]
fn a_reading_is_summarised_in_one_french_line() {
    assert_eq!(resume_fr(Some(&reference())), "direct — RC Lens – Paris FC, 1-0, 34:12");
    assert_eq!(resume_fr(None), "image non analysée");
    assert_eq!(resume_fr(Some(&reading("resume", "", "", "", "", 0.9))), "résumé / bande promo (pas un direct)");
}

#[test]
fn a_recognised_match_forms_one_group_apart_from_its_summary() {
    let live = reference();
    let promo = reading("resume", "RC Lens", "Paris FC", "", "", 0.9);

    assert_eq!(group_key(Some(&live), "a"), group_key(Some(&live), "b"));
    assert_ne!(group_key(Some(&live), "a"), group_key(Some(&promo), "a"));
    assert_eq!(group_key(None, "p:3"), GroupKey::Slot("p:3".into()));
}
