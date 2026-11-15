use std::collections::HashMap;

use super::{team_names, Row, RowState};
use crate::matching::{self, GroupKey, Verdict};

#[derive(Debug, Clone)]
pub struct Member {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub n: usize,
    pub kind: String,
    pub label: String,
    pub content: String,
    pub score: String,
    pub clock: String,
    pub teams: Vec<String>,
    pub verdict: Option<Verdict>,
    pub why: String,
    pub target: Option<Verdict>,
    pub target_why: String,
    pub members: Vec<Member>,
    first: usize,
}

fn rank(verdict: Option<Verdict>) -> i32 {
    verdict.map_or(-1, Verdict::rank)
}

fn slot_of(row: &Row) -> String {
    row.number.map_or_else(|| row.id.clone(), |n| format!("#{n}"))
}

fn order(group: &Group) -> u8 {
    match rank(group.verdict).max(rank(group.target)) {
        3 => 0,
        2 => 1,
        _ if group.kind == "direct" => 2,
        _ => 3,
    }
}

pub fn build_groups(rows: &[Row]) -> Vec<Group> {
    let mut keys: Vec<GroupKey> = Vec::new();
    let mut buckets: HashMap<GroupKey, Vec<&Row>> = HashMap::new();
    for row in rows.iter().filter(|r| r.state == RowState::Ok) {
        let key = matching::group_key(row.reading.as_ref(), &slot_of(row));
        if !buckets.contains_key(&key) {
            keys.push(key.clone());
        }
        buckets.entry(key).or_default().push(row);
    }
    let mut groups: Vec<Group> = keys
        .iter()
        .map(|key| {
            let members = &buckets[key];
            let mut picks: Vec<&Row> = members.clone();
            picks.sort_by_key(|r| (-rank(r.verdict), -rank(r.target), r.reading.as_ref().is_none_or(|f| f.kind != "direct"), r.index));
            let rep = picks[0];
            let reading = rep.reading.as_ref();
            let kind = reading.map_or("inconnu", |f| f.kind.as_str()).to_string();
            Group {
                n: 0,
                label: matching::type_label(&kind).to_string(),
                content: if reading.is_some() { matching::resume_fr(reading) } else if rep.why.is_empty() { "image non analysée".into() } else { rep.why.clone() },
                kind,
                score: reading.map(|f| f.score.clone()).unwrap_or_default(),
                clock: reading.map(|f| f.clock.clone()).unwrap_or_default(),
                teams: reading.map(team_names).unwrap_or_default(),
                verdict: rep.verdict,
                why: rep.why.clone(),
                target: rep.target,
                target_why: rep.target_why.clone(),
                members: picks.iter().map(|r| Member { id: r.id.clone(), name: r.name.clone() }).collect(),
                first: members.iter().map(|r| r.index).min().unwrap_or(0),
            }
        })
        .collect();
    groups.sort_by_key(|g| (order(g), g.first));
    for (i, group) in groups.iter_mut().enumerate() {
        group.n = i + 1;
    }
    groups
}
