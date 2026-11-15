use super::support::{extracted, paris_monaco, Run, NOW_MS};
use crate::live_match::{period_k, stoppage_second, IMAGE_MARGIN_MS};

#[test]
fn image_times_follow_the_calibrated_period_start() {
    let none = extracted(&paris_monaco());
    let mut first_only = Run::default();
    first_only.period_k.insert(1, 1_793_549_712_000);
    let one = first_only.extract(&paris_monaco());
    let mut both = Run::default();
    both.period_k
        .extend([(1, 1_793_549_712_000), (2, 1_793_552_900_000)]);
    let two = both.extract(&paris_monaco());
    let at = |m: &crate::live_match::MatchPayload, id: &str| {
        m.facts.iter().find(|f| f.id == id).unwrap().t_image
    };

    assert!(none.facts.iter().all(|f| f.t_image.is_none()));
    assert_eq!(
        none.periods
            .iter()
            .map(|p| (p.number, p.s0, p.t_image))
            .collect::<Vec<_>>(),
        [(1, 0, None), (2, 2700, None)]
    );
    assert_eq!(
        at(&one, "54659513"),
        Some(1_793_549_712_000 + 2250 * 1000 + IMAGE_MARGIN_MS)
    );
    assert_eq!(at(&one, "54664259"), None);
    assert_eq!(
        at(&two, "54664259"),
        Some(1_793_552_900_000 + 1103 * 1000 + IMAGE_MARGIN_MS)
    );
}

#[test]
fn facts_of_an_uncalibrated_period_anchor_on_the_next_calibrated_one() {
    let k2 = 1_793_739_947_026;
    let mut run = Run::default();
    run.period_k.insert(2, k2);

    let m = run.extract(&paris_monaco());

    let first: Vec<_> = m.facts.iter().filter(|f| f.period == 1).collect();
    let second: Vec<_> = m
        .facts
        .iter()
        .filter(|f| f.period == 2 && !f.minute.contains('+'))
        .collect();
    assert!(!first.is_empty() && first.iter().all(|f| f.t_image == Some(k2)));
    assert!(
        !second.is_empty()
            && second
                .iter()
                .all(|f| f.t_image == Some(k2 + (f.s - 2700) * 1000 + IMAGE_MARGIN_MS))
    );
}

#[test]
fn a_period_without_a_number_is_deduced_from_the_clock() {
    let mut summary = paris_monaco();
    for item in summary["commentary"].as_array_mut().unwrap() {
        if let Some(play) = item.get_mut("play") {
            play.as_object_mut().unwrap().remove("period");
        }
    }
    let mut run = Run::default();
    run.period_k.extend([(1, 0), (2, 10_000_000)]);

    let m = run.extract(&summary);

    let at = |id: &str| m.facts.iter().find(|f| f.id == id).unwrap().t_image;
    assert_eq!(
        (at("54659513"), at("54664259")),
        (
            Some(2250 * 1000 + IMAGE_MARGIN_MS),
            Some(10_000_000 + 1103 * 1000 + IMAGE_MARGIN_MS)
        )
    );
}

#[test]
fn the_first_sight_of_a_fact_is_remembered() {
    let mut run = Run::default();
    run.seen.insert("54659513".into(), 111);
    let first = run.extract(&paris_monaco());
    run.now_s = Some(super::support::NOW_S + 20.0);

    let later = run.extract(&paris_monaco());

    let seen = |m: &crate::live_match::MatchPayload, id: &str| {
        m.facts.iter().find(|f| f.id == id).unwrap().t_vu
    };
    assert_eq!(
        (seen(&first, "54659513"), seen(&first, "54664259")),
        (111, NOW_MS)
    );
    assert_eq!(seen(&later, "54664259"), NOW_MS);
}

#[test]
fn possession_snapshots_are_deduplicated_and_capped() {
    let mut run = Run::default();
    let first = run.extract(&paris_monaco());
    run.possession = first.possession.clone();
    let unchanged = run.extract(&paris_monaco());
    let mut changed = paris_monaco();
    for team in changed["boxscore"]["teams"].as_array_mut().unwrap() {
        for stat in team["statistics"].as_array_mut().unwrap() {
            if stat["name"] == "possessionPct" {
                stat["displayValue"] = "50.0".into();
            }
        }
    }
    let after = run.extract(&changed);
    run.possession = (0..15)
        .map(|i| crate::live_match::Possession {
            t_image: i,
            t_vu: i,
            dom: 40.0 + i as f64 / 100.0,
            ext: 60.0 - i as f64 / 100.0,
        })
        .collect();

    let long = run.extract(&paris_monaco());

    assert_eq!(
        first
            .possession
            .iter()
            .map(|p| (p.t_image, p.t_vu, p.dom, p.ext))
            .collect::<Vec<_>>(),
        [(NOW_MS + 60_000, NOW_MS, 44.4, 55.6)]
    );
    assert_eq!(unchanged.possession, first.possession);
    assert_eq!(
        after.possession.iter().map(|p| p.dom).collect::<Vec<_>>(),
        [44.4, 50.0]
    );
    assert_eq!(long.possession.len(), 10);
}

#[test]
fn added_time_reads_the_label_not_the_capped_clock() {
    let k2 = 1_793_739_947_026;
    let mut run = Run::default();
    run.period_k.insert(2, k2);

    let m = run.extract(&paris_monaco());

    let added: Vec<_> = m.facts.iter().filter(|f| f.minute.contains('+')).collect();
    assert_eq!(
        (stoppage_second("45'+3'"), stoppage_second("90'+4'")),
        (Some((2880, 1)), Some((5640, 2)))
    );
    assert_eq!(
        (
            stoppage_second("105'+1'"),
            stoppage_second("120'+2'"),
            stoppage_second("67'"),
            stoppage_second("")
        ),
        (Some((6360, 3)), Some((7320, 4)), None, None)
    );
    assert!(["90'+2'", "90'+4'", "90'+6'"]
        .iter()
        .all(|m| added.iter().any(|f| f.minute == *m)));
    for fact in added {
        let n: i64 = fact
            .minute
            .split('+')
            .nth(1)
            .unwrap()
            .trim_end_matches('\'')
            .parse()
            .unwrap();
        assert_eq!(
            (fact.s, fact.period, fact.t_image),
            (
                5400,
                2,
                Some(k2 + (5400 + n * 60 - 2700) * 1000 + IMAGE_MARGIN_MS)
            )
        );
    }
}

#[test]
fn extra_time_and_penalties_add_their_periods() {
    let mut summary = paris_monaco();
    let periods = |s: &serde_json::Value| {
        extracted(s)
            .periods
            .iter()
            .map(|p| p.number)
            .collect::<Vec<_>>()
    };
    summary["header"]["competitions"][0]["status"]["period"] = 3.into();
    let third = periods(&summary);
    summary["header"]["competitions"][0]["status"]["period"] = 5.into();

    assert_eq!(third, [1, 2, 3]);
    assert_eq!(periods(&summary), [1, 2, 3, 4, 5]);
}

#[test]
fn the_period_start_is_the_median_of_the_readings() {
    assert_eq!(period_k(&[]), None);
    assert_eq!(period_k(&[(1000.0, 100.0)]), Some(900_000.0));
    assert_eq!(
        period_k(&[(1000.0, 100.0), (1060.0, 160.0), (1130.0, 100.0)]),
        Some(900_000.0)
    );
    assert_eq!(
        period_k(&[(1000.0, 100.0), (1000.0, 110.0)]),
        Some(895_000.0)
    );
}
