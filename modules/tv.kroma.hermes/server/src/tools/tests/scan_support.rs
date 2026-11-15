use crate::matching::Verdict;
use crate::model::ScreenReading;
use crate::scans::{build_groups, Row, RowState, Scan};
use crate::testing::Rig;

pub fn reading(kind: &str, team1: &str, team2: &str, score: &str, clock: &str) -> ScreenReading {
    let mut r = ScreenReading::unknown();
    r.kind = kind.into();
    r.team1 = team1.into();
    r.team2 = team2.into();
    r.score = score.into();
    r.clock = clock.into();
    r.confidence = 0.9;
    r
}

pub fn row(index: usize, id: &str, reading: Option<ScreenReading>, verdict: Option<Verdict>) -> Row {
    Row { index, id: id.into(), name: format!("Chaîne {id}"), number: Some(index as u32 + 1), state: RowState::Ok, reason: String::new(), reading, verdict, why: String::new(), target: None, target_why: String::new() }
}

pub fn remember(rig: &Rig, id: u32, live: &str, compared: bool, rows: Vec<Row>) {
    let groups = build_groups(&rows);
    rig.deps.scans.add(Scan { id, at: rig.now(), provider: "backup".into(), query: "ligue1+".into(), compared, search: String::new(), live: Some(live.into()), by: "u-mamie".into(), ref_teams: vec!["lens".into(), "paris fc".into()], ref_at: Some(rig.now()), rows, groups });
}

pub fn remember_scan(rig: &Rig, id: u32, channel: &str) {
    remember(rig, id, "tf1", false, vec![row(0, channel, Some(reading("direct", "Lens", "Paris FC", "1-0", "34:12")), None)]);
}
