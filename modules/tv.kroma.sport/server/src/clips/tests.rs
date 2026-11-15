use std::collections::BTreeMap;

use serde_json::json;

use super::{
    clip_id, review, to_cut, GoalState, AFTER_S, ATTEMPTS_MAX, BEFORE_S, BEFORE_STOPPAGE_S,
    FALLBACK_AFTER_S, FALLBACK_BEFORE_S,
};
use crate::iso::parse_secs;
use crate::live_match::{Fact, MatchPayload, Period, Player, TeamInfo, Teams};

const T: f64 = 1_800_000_000.0;

fn goal(
    id: &str,
    side: &str,
    (period, s): (i64, i64),
    minute: &str,
    player: &str,
    (t_image, t_vu): (Option<f64>, f64),
) -> Fact {
    Fact {
        id: id.into(),
        kind: "but".into(),
        side: side.into(),
        s,
        period,
        minute: minute.into(),
        player: Some(player.into()),
        t_image: t_image.map(|t| (t * 1000.0) as i64),
        t_vu: (t_vu * 1000.0) as i64,
        ..Fact::default()
    }
}

fn the_match() -> MatchPayload {
    let team = |name: &str| TeamInfo {
        name: name.into(),
        ..TeamInfo::default()
    };
    let player = |id: &str, name: &str| Player {
        id: id.into(),
        name: name.into(),
        ..Player::default()
    };
    MatchPayload {
        id: "espn-401".into(),
        state: "in".into(),
        competition: "Ligue 1".into(),
        teams: Teams {
            dom: team("Paris FC"),
            ext: team("AS Monaco"),
        },
        players: vec![player("9", "Kebbal"), player("7", "Minamino")],
        periods: vec![
            Period {
                number: 1,
                s0: 0,
                t_image: Some(((T - 1500.0) * 1000.0) as i64),
            },
            Period {
                number: 2,
                s0: 2700,
                t_image: None,
            },
        ],
        facts: vec![
            goal(
                "a1",
                "dom",
                (1, 1200),
                "20'",
                "9",
                (Some(T - 300.0), T - 290.0),
            ),
            Fact {
                id: "c1".into(),
                kind: "corner".into(),
                side: "ext".into(),
                period: 1,
                s: 1300,
                ..Fact::default()
            },
            goal("a2", "ext", (2, 3000), "50'", "7", (None, T - 30.0)),
        ],
        ..MatchPayload::default()
    }
}

fn summary() -> serde_json::Value {
    json!({"keyEvents": [{"id": "a2", "wallclock": wallclock(T - 200.0)}]})
}

fn wallclock(secs: f64) -> String {
    let days = (secs / 86_400.0).floor() as i64;
    let rest = secs as i64 - days * 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn none() -> BTreeMap<String, f64> {
    BTreeMap::new()
}

fn states(
    m: &MatchPayload,
    now: f64,
    done: &BTreeMap<String, f64>,
    attempts: &BTreeMap<String, u32>,
) -> Vec<(String, GoalState)> {
    review(m, &summary(), now, 60.0, done, attempts, None)
        .into_iter()
        .map(|r| (r.id, r.state))
        .collect()
}

#[test]
fn a_goal_calibrated_on_the_image_is_cut_around_its_image_time_with_its_label() {
    let todo = to_cut(review(
        &the_match(),
        &json!({}),
        T,
        60.0,
        &none(),
        &BTreeMap::new(),
        None,
    ));

    assert_eq!(
        todo.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(),
        ["espn-401-but-a1"]
    );
    let window = todo[0].window.unwrap();
    assert!(window.from_image);
    assert_eq!(
        (window.from_s, window.to_s),
        (T - 300.0 - BEFORE_S, T - 300.0 + AFTER_S)
    );
    assert!(window.from_s < window.pivot_s && window.pivot_s < window.to_s);
    let label = &todo[0].label;
    assert_eq!(
        (
            label.dom.as_str(),
            label.ext.as_str(),
            label.minute.as_str(),
            label.equipe.as_str(),
            label.buteur.as_str(),
            label.score.as_str()
        ),
        ("Paris FC", "AS Monaco", "20'", "dom", "Kebbal", "1-0")
    );
}

#[test]
fn too_early_out_of_the_buffer_done_or_given_up_is_not_cut() {
    let m = the_match();
    let a1 = "espn-401-but-a1".to_string();
    let done = BTreeMap::from([(a1.clone(), T)]);
    let attempts = BTreeMap::from([(a1.clone(), ATTEMPTS_MAX)]);
    let pre = MatchPayload {
        state: "pre".into(),
        ..the_match()
    };

    assert_eq!(
        states(&m, T - 300.0 + AFTER_S + 2.0, &none(), &BTreeMap::new())[0].1,
        GoalState::TooEarly
    );
    assert_eq!(
        states(&m, T + 400.0, &none(), &BTreeMap::new())[0].1,
        GoalState::OutOfBuffer
    );
    assert_eq!(states(&m, T, &done, &BTreeMap::new())[0].1, GoalState::Done);
    assert_eq!(states(&m, T, &none(), &attempts)[0].1, GoalState::GaveUp);
    assert!(states(&pre, T, &none(), &BTreeMap::new()).is_empty());
}

#[test]
fn the_real_buffer_start_replaces_the_default_useful_span() {
    let m = the_match();
    let reviews = |from: Option<f64>| {
        review(
            &m,
            &json!({}),
            T + 400.0,
            60.0,
            &none(),
            &BTreeMap::new(),
            from,
        )[0]
        .state
    };

    assert_eq!(reviews(None), GoalState::OutOfBuffer);
    assert_eq!(reviews(Some(T - 400.0)), GoalState::ToCut);
}

#[test]
fn an_uncalibrated_period_falls_back_to_the_espn_time_plus_the_delay() {
    let done = BTreeMap::from([("espn-401-but-a1".to_string(), T)]);

    let todo = to_cut(review(
        &the_match(),
        &summary(),
        T,
        60.0,
        &done,
        &BTreeMap::new(),
        None,
    ));
    let no_time = to_cut(review(
        &the_match(),
        &json!({}),
        T,
        60.0,
        &done,
        &BTreeMap::new(),
        None,
    ));

    assert_eq!(todo.len(), 1);
    let window = todo[0].window.unwrap();
    assert!(!window.from_image);
    assert_eq!(window.pivot_s, T - 200.0 + 60.0);
    assert_eq!(
        window.to_s - window.from_s,
        FALLBACK_BEFORE_S + FALLBACK_AFTER_S
    );
    assert_eq!(
        (todo[0].label.score.as_str(), todo[0].label.buteur.as_str()),
        ("1-1", "Minamino")
    );
    assert!(no_time.is_empty());
}

#[test]
fn added_time_widens_the_window_before_the_goal() {
    let mut m = the_match();
    m.facts[0].minute = "45'+3'".into();

    let todo = to_cut(review(
        &m,
        &json!({}),
        T,
        60.0,
        &none(),
        &BTreeMap::new(),
        None,
    ));

    assert_eq!(
        todo[0].window.unwrap().from_s,
        T - 300.0 - BEFORE_STOPPAGE_S
    );
}

#[test]
fn the_identifier_is_safe_for_a_file_name() {
    assert_eq!(clip_id("espn-401", "54/..\\x"), "espn-401-but-54-x");
    assert_eq!(parse_secs(&wallclock(T)), Some(T));
}
