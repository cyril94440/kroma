use super::support::{havre, id, lens, matches, people, pronos, t0};
use crate::prono::{final_text, find_for_prono, Effect, Status, ZAPPED_AFTER_S};

#[test]
fn a_prono_opens_closes_at_kickoff_and_counts_points_at_the_whistle() {
    let mut p = pronos();
    let opening = matches(&[havre()]);
    let required = people(&["Cyril", "Ced"]);
    let effect = p.cycle(
        Some(&opening),
        opening.first(),
        "FR: LIGUE1+ 1",
        &required,
        t0(),
    );
    let opened = p.payload(t0()).unwrap();
    let votes = [
        ("Cyril", "0-2", 10.0),
        ("Ced", "0-3", 20.0),
        ("Mamie", "1-1", 30.0),
    ]
    .map(|(n, s, dt)| p.vote(&id(n), n, "espn-401", s, t0() + dt));
    let late_votes = [
        ("Ced", "16-0", "espn-401"),
        ("Ced", "2", "espn-401"),
        ("Ced", "1-0", "autre"),
    ]
    .map(|(n, s, m)| p.vote(&id(n), n, m, s, t0() + 40.0).0);
    let spread = p.payload(t0() + 40.0).unwrap();
    let kickoff = matches(&[havre().at("in", "1'", 1)]);

    let closing = p.follow(kickoff.first(), true, t0() + 300.0);
    let after_close = p.vote("u-papa", "Papa", "espn-401", "0-0", t0() + 301.0).0;
    let playing = matches(&[havre().at("in", "67'", 2).scored("0", "2")]);
    let live = p.follow(playing.first(), true, t0() + 4000.0);
    let live_payload = p.payload(t0() + 4000.0).unwrap();
    let whistle = matches(&[havre().at("post", "90'", 2).scored("0", "3")]);
    let finish = p.follow(whistle.first(), true, t0() + 6600.0);
    let result = p.payload(t0() + 6600.0).unwrap();
    let again = p.follow(whistle.first(), true, t0() + 6700.0);

    assert_eq!(effect, [Effect::Opened]);
    assert_eq!(
        (
            opened["statut"].clone(),
            opened["ferme_dans_s"].clone(),
            opened["auto"].clone(),
            opened["chaine"].clone(),
            opened["home_abbr"].clone()
        ),
        (
            "ouvert".into(),
            600.into(),
            true.into(),
            "FR: LIGUE1+ 1".into(),
            "HAC".into()
        )
    );
    assert_eq!(votes.map(|v| v.0), [true, true, true]);
    assert!(spread["requis"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "u-mamie"));
    assert_eq!(late_votes, [false, false, false]);
    assert_eq!(
        spread["distribution"],
        serde_json::json!([{"score": "0-2", "n": 1}, {"score": "0-3", "n": 1}, {"score": "1-1", "n": 1}])
    );
    assert_eq!(closing, [Effect::Updated, Effect::Closed]);
    assert!(!after_close);
    assert_eq!(live, [Effect::Updated]);
    assert_eq!(
        (
            live_payload["score"].clone(),
            live_payload["minute"].clone(),
            live_payload["classement_match"][0]["pseudo"].clone()
        ),
        ("0-2".into(), "67'".into(), "Cyril".into())
    );
    assert_eq!(finish, [Effect::Updated, Effect::Finished]);
    let rows: Vec<_> = result["resultats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["pseudo"].as_str().unwrap(),
                r["points"].as_u64().unwrap(),
                r["exact"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [("Ced", 3, true), ("Cyril", 1, false), ("Mamie", 0, false)]
    );
    assert_eq!(
        result["saison"][0],
        serde_json::json!({"user_id": "u-ced", "pseudo": "Ced", "points": 3, "gagnes": 1})
    );
    assert_eq!(
        final_text(p.current.as_ref().unwrap()).unwrap().key,
        "sport.announce.prono.finalExact"
    );
    assert!(again.is_empty());
    assert_eq!(p.season["u-ced"].points, 3);
}

#[test]
fn opened_mid_match_it_closes_at_the_tenth_minute() {
    let mut p = pronos();
    let six = matches(&[havre().at("in", "6'", 1)]);
    p.cycle(Some(&six), six.first(), "c", &people(&["A"]), t0() + 400.0);

    let remaining = p.payload(t0() + 400.0).unwrap()["ferme_dans_s"].clone();
    let voted = p.vote("u-a", "A", "espn-401", "1-0", t0() + 500.0).0;
    let eight = matches(&[havre().at("in", "8'", 1)]);
    let updated = p.follow(eight.first(), true, t0() + 520.0);
    let late = p.vote("u-a", "A", "espn-401", "2-0", t0() + 700.0).0;
    let ten = matches(&[havre().at("in", "10'", 1)]);
    let closed = p.follow(ten.first(), true, t0() + 700.0);

    assert_eq!((remaining, voted, late), (240.into(), true, false));
    assert_eq!(
        (updated, closed),
        (vec![Effect::Updated], vec![Effect::Updated, Effect::Closed])
    );
}

#[test]
fn one_prono_per_match_and_none_during_another() {
    let mut p = pronos();
    let both = matches(&[havre(), lens()]);
    let detected = both.iter().find(|m| m.id == "espn-401");
    p.cycle(Some(&both), detected, "c", &[], t0());

    let during = p.cycle(
        Some(&both),
        both.iter().find(|m| m.id == "espn-402"),
        "c",
        &[],
        t0() + 5.0,
    );
    let post = matches(&[havre().at("post", "90'", 2).scored("1", "1")]);
    p.follow(post.first(), true, t0() + 7000.0);
    let same_again = p.cycle(Some(&post), post.first(), "c", &[], t0() + 7100.0);
    let evening = matches(&[havre().at("post", "90'", 2), lens().on("2026-10-30T20:00Z")]);
    let other = p.cycle(
        Some(&evening),
        evening.iter().find(|m| m.id == "espn-402"),
        "c",
        &[],
        t0() + 7200.0,
    );

    assert!(during.is_empty() && same_again.is_empty());
    assert_eq!(other, [Effect::Opened]);
}

#[test]
fn a_prono_nobody_played_clears_when_another_match_is_on_screen() {
    let mut p = pronos();
    let both = matches(&[havre().at("in", "5'", 1), lens().at("in", "5'", 1)]);
    let (first, second) = (
        both.iter().find(|m| m.id == "espn-401"),
        both.iter().find(|m| m.id == "espn-402"),
    );
    p.cycle(
        Some(&both),
        first,
        "CANAL+ LIVE 4",
        &people(&["Cyril"]),
        t0(),
    );

    let same_channel = p.cycle(Some(&both), None, "CANAL+ LIVE 4", &[], t0() + 60.0);
    let same_match_elsewhere = p.cycle(Some(&both), first, "SECOURS CANAL 4", &[], t0() + 70.0);
    let waiting = p.cycle(Some(&both), None, "CANAL+ FOOT", &[], t0() + 80.0);
    let kept = p.active();
    let zapped = p.cycle(Some(&both), second, "CANAL+ FOOT", &[], t0() + 90.0);
    let cancelled = p.payload(t0() + 91.0).unwrap();
    let reopened = p.cycle(
        Some(&both),
        second,
        "CANAL+ FOOT",
        &people(&["Cyril"]),
        t0() + 150.0,
    );
    p.vote("u-cyril", "Cyril", "espn-402", "2-1", t0() + 160.0);
    let played = p.cycle(Some(&both), first, "TF1", &[], t0() + 170.0);

    assert!(
        !same_channel.contains(&Effect::Away)
            && !same_match_elsewhere.contains(&Effect::Away)
            && !waiting.contains(&Effect::Away)
            && kept
    );
    assert_eq!(zapped, [Effect::Away]);
    assert!(cancelled["annule"] == true && !p.is_seen("espn-401"));
    assert_eq!(
        (reopened, p.current.as_ref().unwrap().id.clone()),
        (vec![Effect::Opened], "espn-402".to_string())
    );
    assert!(!played.contains(&Effect::Away) && p.current.as_ref().unwrap().id == "espn-402");
}

#[test]
fn a_prono_never_played_goes_away_after_three_minutes_without_a_recognised_match() {
    let mut p = pronos();
    let live = matches(&[havre().at("in", "5'", 1)]);
    p.cycle(Some(&live), live.first(), "A", &people(&["Cyril"]), t0());

    let first = p.cycle(Some(&live), None, "TF1", &[], t0() + 60.0);
    let later = p.cycle(Some(&live), None, "TF1", &[], t0() + 60.0 + ZAPPED_AFTER_S);

    assert!(!first.contains(&Effect::Away));
    assert_eq!(later, [Effect::Away]);
}

#[test]
fn a_match_that_vanished_is_abandoned_without_points_and_a_silent_source_changes_nothing() {
    let mut p = pronos();
    let live = matches(&[havre().at("in", "5'", 1)]);
    p.cycle(Some(&live), live.first(), "c", &[], t0());
    p.vote("u-a", "A", "espn-401", "1-0", t0() + 1.0);

    let waiting: Vec<_> = (0..14)
        .map(|i| p.cycle(Some(&[]), None, "c", &[], t0() + 100.0 + f64::from(i)))
        .collect();
    let abandoned = p.cycle(Some(&[]), None, "c", &[], t0() + 200.0);
    let mut silent = pronos();
    silent.cycle(Some(&live), live.first(), "c", &[], t0());
    (0..40).for_each(|i| {
        silent.cycle(None, None, "c", &[], t0() + 100.0 + f64::from(i));
    });

    assert!(waiting.iter().all(Vec::is_empty));
    assert_eq!(abandoned, [Effect::Abandoned]);
    assert!(p.current.is_none() && p.season.is_empty());
    assert_eq!(silent.current.as_ref().map(|c| c.missed), Some(0));
}

#[test]
fn a_prono_on_request_follows_the_same_windows_with_six_hours_ahead() {
    let board = matches(&[havre(), lens().at("in", "40'", 1)]);
    let p = pronos();
    let mut played = pronos();
    played.seen.push("espn-401".into());

    let found = find_for_prono("PSG", "Le Havre", &board, t0(), &p).map(|m| m.id.clone());
    let reasons = [
        find_for_prono("Lens", "Paris FC", &board, t0(), &p).err(),
        find_for_prono("Nantes", "Metz", &board, t0(), &p).err(),
        find_for_prono("PSG", "Le Havre", &board, t0(), &played).err(),
        find_for_prono("PSG", "Le Havre", &board, t0() - 7.0 * 3600.0, &p).err(),
        find_for_prono("Lens", "PSG", &board, t0(), &p).err(),
    ];

    assert_eq!(found, Ok("espn-401".to_string()));
    assert_eq!(
        reasons,
        [
            Some("tropAvance"),
            Some("matchIntrouvable"),
            Some("dejaJoue"),
            Some("tropLointain"),
            Some("matchIntrouvable")
        ]
    );
    assert_eq!(p.current.as_ref().map(|c| c.status), None::<Status>);
}
