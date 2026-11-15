use std::collections::BTreeMap;

use serde_json::Value;

use super::json::{id_of, id_text, number, text};
use super::model::{Fact, Possession, Teams};
use super::periods::{contract_type, period_start_s, stoppage_second, IMAGE_MARGIN_MS};
use super::players::PlayerIndex;
use crate::text::norm_lossy;

pub struct Carry<'a> {
    pub seen: &'a mut BTreeMap<String, i64>,
    pub period_k: &'a BTreeMap<i64, i64>,
    pub possession: &'a [Possession],
}

fn period_of(
    play: &Value,
    s: f64,
    second_half_start: Option<&str>,
    from_stoppage: Option<i64>,
) -> i64 {
    if let Some(n) = play
        .get("period")
        .and_then(|p| p.get("number"))
        .and_then(Value::as_i64)
        .filter(|n| *n > 0)
    {
        return n;
    }
    if let Some(p) = from_stoppage.filter(|p| *p > 0) {
        return p;
    }
    let wall = text(play, "wallclock");
    if s >= 2700.0 || second_half_start.is_some_and(|start| wall.as_str() >= start) {
        2
    } else {
        1
    }
}

pub(super) struct Sides {
    pub by_team_id: BTreeMap<String, &'static str>,
    pub by_name: BTreeMap<String, &'static str>,
}

pub(super) fn sides(home: &Value, away: &Value, teams: &Teams) -> Sides {
    let mut by_name: BTreeMap<String, &'static str> = BTreeMap::from([
        (norm_lossy(&teams.dom.name), "dom"),
        (norm_lossy(&teams.ext.name), "ext"),
    ]);
    for (competitor, side) in [(home, "dom"), (away, "ext")] {
        let team = competitor.get("team").cloned().unwrap_or(Value::Null);
        for key in ["shortDisplayName", "name", "location"] {
            let name = text(&team, key);
            if !name.is_empty() {
                by_name.entry(norm_lossy(&name)).or_insert(side);
            }
        }
    }
    let by_team_id = BTreeMap::from([
        (id_text(home.get("team").and_then(|t| t.get("id"))), "dom"),
        (id_text(away.get("team").and_then(|t| t.get("id"))), "ext"),
    ]);
    Sides {
        by_team_id,
        by_name,
    }
}

pub(super) fn fact_of(
    item: &Value,
    ctx: &PlayContext,
    carry: &mut Carry,
    now_ms: i64,
) -> Option<Fact> {
    let play = item.get("play")?;
    let id = id_of(play.get("id"))?;
    let label = text(play.get("type").unwrap_or(&Value::Null), "text");
    let kind = contract_type(&label)?;
    let participants: Vec<(Option<String>, Option<String>)> = play
        .get("participants")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .map(|p| {
                    let athlete = p.get("athlete").cloned().unwrap_or(Value::Null);
                    let name = Some(text(&athlete, "displayName"))
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| text(&athlete, "fullName"));
                    ctx.index
                        .get(&norm_lossy(&name))
                        .map_or((None, None), |(id, side)| {
                            (Some(id.clone()), Some(side.clone()))
                        })
                })
                .collect()
        })
        .unwrap_or_default();
    let team = play.get("team").cloned().unwrap_or(Value::Null);
    let mut side: Option<String> = ctx
        .sides
        .by_team_id
        .get(&id_text(team.get("id")))
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.sides
                .by_name
                .get(&norm_lossy(&text(&team, "displayName")))
                .map(|s| s.to_string())
        });
    let first_side = participants.first().and_then(|p| p.1.clone());
    if side.is_none() {
        side = first_side.clone();
    }
    if label == "Own Goal" {
        if let Some(own) = &first_side {
            side = Some(if own == "dom" { "ext" } else { "dom" }.into());
        }
    }
    let side = side?;
    let clock = play.get("clock").cloned().unwrap_or(Value::Null);
    let seconds = number(clock.get("value")).unwrap_or(0.0);
    let stoppage = stoppage_second(&text(&clock, "displayValue"));
    let image_seconds = stoppage.map_or(seconds, |(s, _)| s as f64);
    let period = period_of(
        play,
        seconds,
        ctx.second_half_start.as_deref(),
        stoppage.map(|(_, p)| p),
    );
    let t_image = match carry.period_k.get(&period) {
        Some(k) => Some(
            *k + ((image_seconds - period_start_s(period).unwrap_or(2700) as f64) * 1000.0) as i64
                + IMAGE_MARGIN_MS,
        ),
        None => (period + 1..6).find_map(|q| carry.period_k.get(&q).copied()),
    };
    let mut fact = Fact {
        id: id.clone(),
        kind: kind.into(),
        side,
        s: seconds as i64,
        period,
        minute: text(&clock, "displayValue").chars().take(10).collect(),
        x: number(play.get("fieldPositionX")),
        y: number(play.get("fieldPositionY")),
        t_image,
        t_vu: *carry.seen.entry(id).or_insert(now_ms),
        ..Fact::default()
    };
    if kind == "remplacement" {
        fact.came_on = participants.first().and_then(|p| p.0.clone());
        fact.went_off = participants.get(1).and_then(|p| p.0.clone());
    } else {
        fact.player = participants.first().and_then(|p| p.0.clone());
        if kind == "but" && label != "Own Goal" && participants.len() > 1 {
            fact.assist = participants[1].0.clone();
        }
    }
    Some(fact)
}

pub(super) struct PlayContext<'a> {
    pub index: &'a PlayerIndex,
    pub sides: &'a Sides,
    pub second_half_start: Option<String>,
}
