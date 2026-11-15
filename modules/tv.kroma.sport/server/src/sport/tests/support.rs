use serde_json::Value;

use crate::sport::{Line, Reading, ScanRow, Sport, SportEvent, State};

pub const NOW: f64 = 1_793_880_000.0;

pub fn fixture(name: &str) -> Value {
    let text = match name {
        "foot" => include_str!("../../../fixtures/espn_sport_foot.json"),
        "tennis" => include_str!("../../../fixtures/espn_sport_tennis.json"),
        "f1" => include_str!("../../../fixtures/espn_sport_f1.json"),
        "golf" => include_str!("../../../fixtures/espn_sport_golf.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).unwrap()
}

pub fn event(id: &str, a: &str, b: &str, state: State, start: f64) -> SportEvent {
    SportEvent {
        id: id.into(),
        espn: id.split(':').nth(1).unwrap().into(),
        sport: Sport::Soccer,
        code: "fra.1".into(),
        competition: "Ligue 1".into(),
        state,
        start,
        end: None,
        popularity: 100,
        lines: vec![
            Line {
                name: a.into(),
                score: "1".into(),
            },
            Line {
                name: b.into(),
                score: "0".into(),
            },
        ],
        abbreviations: vec!["RCL".into(), "LIL".into()],
        clock: "30'".into(),
        tournament: None,
    }
}

pub fn live_match() -> SportEvent {
    event("foot:1", "Lens", "Lille", State::In, NOW - 1800.0)
}

pub fn reading(a: &str, b: &str, kind: &str, confidence: f64) -> Reading {
    Reading {
        kind: kind.into(),
        team1: a.into(),
        team2: b.into(),
        confidence,
        score: "1-0".into(),
        ..Reading::default()
    }
}

pub fn scan_row(channel: &str, state: &str, reading: Option<Reading>, num: i64) -> ScanRow {
    ScanRow {
        channel_id: channel.into(),
        name: Some(format!("FR: LIGUE1+ {num} RAW")),
        num: Some(num),
        state: state.into(),
        reading,
        from_guide: false,
    }
}
