use super::support::{havre, matches, pronos, t0};
use crate::prono::{measured_delay, Calibrage, DirectKey, Persisted, Pronos};

const MARGIN: f64 = 15.0;

fn primary() -> DirectKey {
    vec!["primary".into(), "FR: LIGUE1+ 1".into()]
}

fn backup() -> DirectKey {
    vec!["backup".into(), "FR: LIGUE1+ 1".into()]
}

fn measured(values: &[f64], key: &DirectKey) -> Calibrage {
    let mut c = Calibrage::default();
    c.for_match(Some("m"));
    values
        .iter()
        .for_each(|v| assert!(c.add(*v, t0(), Some(key))));
    c
}

#[test]
fn the_delay_formula_and_the_measure_bounds() {
    let k = 1000.0;

    assert_eq!(measured_delay(k + 650.0, 600.0, k + 660.0, 720.0), 50.0);
    assert_eq!(measured_delay(k + 2810.0, 2760.0, k + 2760.0, 2820.0), 50.0);
    assert!(
        crate::prono::valid_measure(0.0)
            && crate::prono::valid_measure(180.0)
            && !crate::prono::valid_measure(-1.0)
            && !crate::prono::valid_measure(181.0)
    );
    assert_eq!(crate::prono::median(&[3.0, 1.0, 2.0]), Some(2.0));
    assert_eq!(crate::prono::median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));
    assert_eq!(
        crate::prono::robust_median(&[48.0, 50.0, 52.0, 170.0, 500.0, -3.0]),
        Some(50.0)
    );
    assert_eq!(crate::prono::robust_median(&[]), None);
}

#[test]
fn the_effective_delay_is_the_median_plus_a_margin_after_three_measures() {
    let mut c = Calibrage::default();
    c.for_match(Some("m1"));
    let key = primary();
    let before = c.delay(60.0, t0(), Some(&key), MARGIN);
    c.add(40.0, t0(), Some(&key));
    c.add(45.0, t0(), Some(&key));
    let two = c.delay(60.0, t0(), Some(&key), MARGIN);
    c.add(50.0, t0(), Some(&key));

    assert_eq!(before, (60.0, "defaut"));
    assert_eq!(two, (60.0, "defaut"));
    assert_eq!(
        c.delay(60.0, t0(), Some(&key), MARGIN),
        (45.0 + MARGIN, "mesure")
    );
    assert!(
        !c.add(150.0, t0(), Some(&key))
            && !c.add(-5.0, t0(), Some(&key))
            && !c.add(46.0, t0(), None)
    );
    assert_eq!(c.measures, [40.0, 45.0, 50.0]);
}

#[test]
fn the_delay_has_a_floor_and_a_sliding_window() {
    let key = primary();
    let low = measured(&[2.0, 3.0, 4.0], &key);
    let mut long = Calibrage::default();
    long.for_match(Some("m"));
    (50..60).for_each(|r| {
        long.add(f64::from(r), t0(), Some(&key));
    });

    assert_eq!(low.delay(60.0, t0(), Some(&key), MARGIN), (20.0, "mesure"));
    assert_eq!(long.measures, (53..60).map(f64::from).collect::<Vec<_>>());
}

#[test]
fn another_match_on_the_same_live_starts_from_the_persisted_median_for_a_day() {
    let key = primary();
    let mut c = measured(&[40.0, 45.0, 50.0], &key);

    c.for_match(Some("m2"));
    let next = c.delay(60.0, t0() + 3600.0, Some(&key), MARGIN);
    let stale = c.delay(
        60.0,
        t0() + 3600.0 + 24.0 * 3600.0 + 5000.0,
        Some(&key),
        MARGIN,
    );

    assert_eq!((c.measures.len(), next), (0, (45.0 + MARGIN, "persistee")));
    assert_eq!(stale.1, "defaut");
}

#[test]
fn measures_belong_to_the_key_of_the_live() {
    let (a, b) = (primary(), backup());
    let mut c = measured(&[80.0, 80.0, 80.0], &a);

    let same = c.delay(60.0, t0(), Some(&a), MARGIN);
    let other = c.delay(60.0, t0(), Some(&b), MARGIN);
    let unknown = c.delay(60.0, t0(), None, MARGIN);
    c.measures = vec![20.0, 20.0, 20.0];
    let low_elsewhere = c.delay(60.0, t0(), Some(&b), MARGIN);
    c.add(90.0, t0() + 1.0, Some(&b));

    assert_eq!(same, (80.0 + MARGIN, "mesure"));
    assert_eq!(
        (other, unknown),
        (
            (80.0 + MARGIN, "mesure_autre_direct"),
            (80.0 + MARGIN, "mesure_autre_direct")
        )
    );
    assert_eq!(low_elsewhere, (60.0, "mesure_autre_direct"));
    assert_eq!((c.measures.clone(), c.key.clone()), (vec![90.0], Some(b)));
}

#[test]
fn the_persisted_median_is_capped_by_the_default_on_another_live() {
    let (a, b) = (primary(), backup());
    let persisted = Calibrage::with_persisted(Some(Persisted {
        median_s: 25.0,
        at: t0(),
        key: Some(a.clone()),
    }));
    let keyless = Calibrage::with_persisted(Some(Persisted {
        median_s: 25.0,
        at: t0(),
        key: None,
    }));

    assert_eq!(
        persisted.delay(60.0, t0() + 10.0, Some(&a), MARGIN),
        (25.0 + MARGIN, "persistee")
    );
    assert_eq!(
        persisted.delay(60.0, t0() + 10.0, Some(&b), MARGIN),
        (60.0, "persistee_autre_direct")
    );
    assert_eq!(
        persisted.delay(60.0, t0() + 10.0, None, MARGIN),
        (60.0, "persistee_autre_direct")
    );
    assert_eq!(
        persisted.delay(30.0, t0() + 10.0, Some(&b), MARGIN),
        (30.0f64.max(25.0 + MARGIN), "persistee_autre_direct")
    );
    assert_eq!(
        keyless.delay(60.0, t0(), Some(&a), MARGIN),
        (60.0, "persistee_autre_direct")
    );
}

#[test]
fn a_change_of_regime_empties_the_window_after_two_rejections_on_the_same_side() {
    let key = primary();
    let mut c = measured(&[40.0, 40.0, 40.0], &key);

    let first = c.add(80.0, t0(), Some(&key));
    let other_side = c.add(3.0, t0(), Some(&key));
    let rejects = c.rejects;
    let again = c.add(80.0, t0(), Some(&key));
    let accepted = c.add(82.0, t0(), Some(&key));
    let partial = c.delay(60.0, t0(), Some(&key), MARGIN);
    let elsewhere = c.delay(
        60.0,
        t0(),
        Some(&vec!["backup".into(), "other".into()]),
        MARGIN,
    );
    c.add(80.0, t0(), Some(&key));
    c.add(81.0, t0(), Some(&key));

    assert_eq!(
        (first, other_side, again, accepted),
        (false, false, false, true)
    );
    assert_eq!(rejects, (-1, 1));
    assert!(c.persisted.is_some());
    assert_eq!(partial, (82.0 + MARGIN, "mesures_partielles"));
    assert_eq!(elsewhere, (60.0, "defaut"));
    assert_eq!(
        c.delay(60.0, t0(), Some(&key), MARGIN),
        (81.0 + MARGIN, "mesure")
    );
}

#[test]
fn the_prono_keeps_valid_measures_of_its_own_match_and_exposes_its_diagnostic() {
    let mut p = pronos();
    p.direct_key = Some(primary());
    let live = matches(&[havre().at("in", "5'", 1)]);
    p.open(
        &live[0],
        "Chaîne",
        &[("u-a".into(), "A".into())],
        true,
        t0(),
    );

    p.current.as_mut().unwrap().status = crate::prono::Status::Running;
    let default = p.delay(t0());
    let rejected = [
        p.measure("espn-401", 400.0, t0()),
        p.measure("espn-other", 50.0, t0() + 200.0),
    ];
    let accepted = [48.0, 50.0, 52.0].map(|r| p.measure("espn-401", r, t0() + 200.0));
    let diagnostic = p.payload(t0() + 200.0).unwrap()["diagnostic"]["anti_spoiler"].clone();

    assert_eq!(default, (60.0, "defaut"));
    assert_eq!((rejected, accepted), ([false, false], [true, true, true]));
    assert_eq!(
        diagnostic,
        serde_json::json!({"delai_s": 65.0, "source": "mesure", "mesures": 3, "mediane_s": 50.0})
    );
}

#[test]
fn the_next_match_starts_on_the_persisted_median_not_the_default() {
    let mut p = pronos();
    p.direct_key = Some(primary());
    let live = matches(&[havre().at("in", "5'", 1)]);
    p.open(&live[0], "c", &[], true, t0());
    [48.0, 50.0, 52.0].iter().for_each(|r| {
        p.measure("espn-401", *r, t0() + 200.0);
    });
    let text = serde_json::to_string(&p.to_file()).unwrap();

    let mut next = Pronos::from_file(serde_json::from_str(&text).unwrap(), 90.0, 15.0);
    next.current = None;
    next.direct_key = Some(primary());
    let same = next.delay(t0() + 300.0);
    next.direct_key = Some(backup());
    let other = next.delay(t0() + 300.0);

    assert_eq!(same, (50.0 + MARGIN, "persistee"));
    assert_eq!(other, (90.0, "persistee_autre_direct"));
    assert_eq!(next.to_file().calibration.map(|c| c.median_s), Some(50.0));
}
