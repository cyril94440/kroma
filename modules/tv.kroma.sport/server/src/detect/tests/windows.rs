use std::collections::BTreeMap;

use super::support::{at, label, matches_at, named, none, programme, synth};
use crate::detect::{decide, merge};

#[test]
fn women_youth_and_reserves_are_dropped_unless_the_guide_says_so() {
    let now = at("19:50", 3);
    let matches = matches_at(now);
    let unannounced = [programme(
        at("19:00", 3),
        at("21:00", 3),
        "Cambridge United - Brighton",
        "cambridge united brighton coupe",
    )];
    let announced = [programme(
        at("19:00", 3),
        at("21:00", 3),
        "Cambridge United - Brighton & Hove Albion U21",
        "cambridge united brighton and hove albion u21 efl trophy",
    )];

    let special =
        |m: &crate::detect::FlatMatch| crate::detect::is_special(&m.competition, &m.home, &m.away);
    let without = decide(&matches, &unannounced, now, None, &none());
    let with = decide(&matches, &announced, now, None, &none());

    assert!(
        special(named(&matches, "Cambridge United"))
            && special(named(&matches, "Aberdeen B"))
            && special(named(&matches, "Charlotte"))
    );
    assert!(!["Villarreal", "Barcelona", "Bodo/Glimt"]
        .iter()
        .any(|h| special(named(&matches, h))));
    assert!(label(&without).is_none_or(|l| !l.contains("Cambridge")));
    assert_eq!(
        label(&with).as_deref(),
        Some("Cambridge United – Brighton & Hove Albion U21")
    );
}

#[test]
fn the_windows_of_pre_in_and_post() {
    let kick = 10_000.0;
    let base = synth("espn-1", "A", "B", ("", ""), "", kick);
    let with = |state: &str, period: i64| crate::detect::FlatMatch {
        state: state.into(),
        period,
        ..base.clone()
    };
    let found = |m: &crate::detect::FlatMatch, now: f64, seen: &BTreeMap<String, f64>| {
        !crate::detect::candidates(std::slice::from_ref(m), now, seen).is_empty()
    };
    let pre = with("pre", 0);
    let post = with("post", 2);
    let seen = BTreeMap::from([("espn-1".to_string(), kick + 7200.0)]);

    assert!(found(&pre, kick - 1800.0, &none()) && !found(&pre, kick - 1860.0, &none()));
    assert!(found(&pre, kick + 900.0, &none()) && !found(&pre, kick + 1260.0, &none()));
    assert!(
        found(&with("in", 1), kick + 3600.0, &none())
            && !found(&with("in", 1), kick + 4.0 * 3600.0, &none())
    );
    assert!(
        found(&post, kick + 115.0 * 60.0 + 840.0, &none())
            && !found(&post, kick + 115.0 * 60.0 + 960.0, &none())
    );
    assert!(found(&post, kick + 7800.0, &seen) && !found(&post, kick + 8400.0, &seen));
    assert!(found(
        &with("post", 4),
        kick + 160.0 * 60.0 + 600.0,
        &none()
    ));
}

#[test]
fn merging_days_keeps_one_match_per_id_and_the_latest_form() {
    let (a, mut b) = (
        synth("espn-1", "A", "B", ("", ""), "", 1.0),
        synth("espn-1", "A2", "B", ("", ""), "", 1.0),
    );
    b.score = "1-0".into();

    let merged = merge(&[
        vec![a, synth("espn-2", "C", "D", ("", ""), "", 1.0)],
        vec![b, synth("espn-3", "E", "F", ("", ""), "", 1.0)],
    ]);

    assert_eq!(
        merged.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["espn-1", "espn-2", "espn-3"]
    );
    assert_eq!(merged[0].home, "A2");
}
