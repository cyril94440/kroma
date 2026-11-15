use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::event::{SportEvent, State};
use super::{other, soccer};
use crate::iso::day_key;

pub const FOOT: [(&str, &str, i32); 12] = [
    ("fra.1", "Ligue 1", 100),
    ("uefa.champions", "Ligue des champions", 95),
    ("fra.coupe_de_france", "Coupe de France", 85),
    ("uefa.europa", "Ligue Europa", 80),
    ("uefa.nations", "Ligue des nations", 80),
    ("fifa.worldq.uefa", "Qualif. Coupe du monde", 78),
    ("eng.1", "Premier League", 75),
    ("esp.1", "Liga", 70),
    ("ita.1", "Serie A", 65),
    ("ger.1", "Bundesliga", 65),
    ("uefa.europa.conf", "Conference League", 60),
    ("fra.2", "Ligue 2", 50),
];

pub const OTHER_PATHS: [(&str, &str); 4] = [
    ("tennis:atp", "/apis/site/v2/sports/tennis/atp/scoreboard"),
    ("tennis:wta", "/apis/site/v2/sports/tennis/wta/scoreboard"),
    ("f1", "/apis/site/v2/sports/racing/f1/scoreboard"),
    ("golf", "/apis/site/v2/sports/golf/pga/scoreboard"),
];

const SIX_HOURS_S: f64 = 6.0 * 3600.0;

pub fn days_to_read(now: f64) -> (Vec<String>, Vec<String>) {
    let near: BTreeSet<String> = [day_key(now), day_key(now - 6.0 * 3600.0)]
        .into_iter()
        .collect();
    (
        near.into_iter().collect(),
        vec![day_key(now + 86_400.0), day_key(now + 2.0 * 86_400.0)],
    )
}

pub fn from_espn(responses: &BTreeMap<String, Value>, now: f64) -> Vec<SportEvent> {
    let mut events = Vec::new();
    for (key, payload) in responses {
        let mut parts = key.split(':');
        if parts.next() != Some("foot") {
            continue;
        }
        let Some((code, name, rank)) = parts
            .next()
            .and_then(|code| FOOT.iter().find(|(c, _, _)| *c == code))
        else {
            continue;
        };
        events.extend(soccer::soccer(payload, code, name, *rank));
    }
    let null = Value::Null;
    let get = |key: &str| responses.get(key).unwrap_or(&null);
    events.extend(other::tennis(get("tennis:atp"), "atp", now));
    events.extend(other::tennis(get("tennis:wta"), "wta", now));
    events.extend(other::f1(get("f1"), now));
    events.extend(other::golf(get("golf")));
    let mut seen = BTreeSet::new();
    events.retain(|e| {
        !(e.state == State::Post && now - e.start > SIX_HOURS_S) && seen.insert(e.id.clone())
    });
    events
}
