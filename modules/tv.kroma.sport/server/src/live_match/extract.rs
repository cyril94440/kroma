use std::collections::BTreeMap;

use serde_json::Value;

use super::facts::{fact_of, sides, Carry, PlayContext, Sides};
use super::json::{id_of, id_text, number, text};
use super::model::{Fact, MatchPayload, Period, Possession, TeamInfo, Teams};
use super::periods::{color, period_start_s, POSSESSION_MAX};
use super::players::{place, players};

pub struct Context<'a> {
    pub delay_s: f64,
    pub now_s: f64,
    pub channel: &'a str,
    pub competition: &'a str,
}

fn team_info(competitor: &Value, roster: Option<&Value>) -> TeamInfo {
    let team = competitor.get("team").cloned().unwrap_or(Value::Null);
    let uniform = roster
        .and_then(|r| r.get("uniform"))
        .map(|u| text(u, "color"))
        .unwrap_or_default();
    TeamInfo {
        name: text(&team, "displayName"),
        abbr: text(&team, "abbreviation").chars().take(4).collect(),
        color: color(&[&uniform, &text(&team, "color")]),
        formation: roster
            .map(|r| text(r, "formation"))
            .unwrap_or_default()
            .chars()
            .take(12)
            .collect(),
    }
}

fn possession_history(
    summary: &Value,
    sides: &Sides,
    state: &str,
    previous: &[Possession],
    now_ms: i64,
    delay_s: f64,
) -> Vec<Possession> {
    let skip = previous.len().saturating_sub(POSSESSION_MAX);
    let mut history: Vec<Possession> = previous[skip..].to_vec();
    let mut values: BTreeMap<&'static str, f64> = BTreeMap::new();
    let teams = summary
        .get("boxscore")
        .and_then(|b| b.get("teams"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for team in teams {
        let side = sides
            .by_team_id
            .get(&id_text(team.get("team").and_then(|t| t.get("id"))));
        let stat = team
            .get("statistics")
            .and_then(Value::as_array)
            .and_then(|s| s.iter().find(|x| text(x, "name") == "possessionPct"))
            .cloned();
        if let (Some(side), Some(stat)) = (side, stat) {
            if let Some(value) = number(stat.get("displayValue")) {
                values.insert(side, value);
            }
        }
    }
    if let (Some(dom), Some(ext)) = (values.get("dom"), values.get("ext")) {
        let changed = history
            .last()
            .is_none_or(|l| (l.dom, l.ext) != (*dom, *ext));
        if dom + ext > 0.0 && state != "pre" && changed {
            history.push(Possession {
                t_image: now_ms + (delay_s * 1000.0) as i64,
                t_vu: now_ms,
                dom: *dom,
                ext: *ext,
            });
            let skip = history.len().saturating_sub(POSSESSION_MAX);
            history.drain(..skip);
        }
    }
    history
}

pub fn extract(summary: &Value, carry: &mut Carry, ctx: &Context) -> MatchPayload {
    let header = summary.get("header").cloned().unwrap_or(Value::Null);
    let competition = header
        .get("competitions")
        .and_then(|c| c.get(0))
        .cloned()
        .unwrap_or(Value::Null);
    let competitors = competition
        .get("competitors")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let side_of = |name: &str| {
        competitors
            .iter()
            .find(|c| text(c, "homeAway") == name)
            .cloned()
            .unwrap_or(Value::Null)
    };
    let (home, away) = (side_of("home"), side_of("away"));
    let now_ms = (ctx.now_s * 1000.0) as i64;
    let status = competition.get("status").cloned().unwrap_or(Value::Null);
    let state = Some(text(status.get("type").unwrap_or(&Value::Null), "state"))
        .filter(|s| ["pre", "in", "post"].contains(&s.as_str()))
        .unwrap_or_else(|| "pre".into());
    let rosters: Vec<Value> = summary
        .get("rosters")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let roster_of = |name: &str| rosters.iter().find(|r| text(r, "homeAway") == name);
    let (mut players, index) = players(&rosters);
    let teams = Teams {
        dom: team_info(&home, roster_of("home")),
        ext: team_info(&away, roster_of("away")),
    };
    let sides = sides(&home, &away, &teams);
    let second_half_start = summary
        .get("keyEvents")
        .and_then(Value::as_array)
        .and_then(|events| {
            events.iter().find(|k| {
                text(k.get("type").unwrap_or(&Value::Null), "text") == "Start 2nd Half"
                    && !text(k, "wallclock").is_empty()
            })
        })
        .map(|k| text(k, "wallclock"));
    place(&mut players, &teams);
    let play_ctx = PlayContext {
        index: &index,
        sides: &sides,
        second_half_start,
    };
    let facts: Vec<Fact> = summary
        .get("commentary")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| fact_of(item, &play_ctx, carry, now_ms))
                .collect()
        })
        .unwrap_or_default();
    let possession = possession_history(
        summary,
        &sides,
        &state,
        carry.possession,
        now_ms,
        ctx.delay_s,
    );
    let status_period = status.get("period").and_then(Value::as_i64).unwrap_or(0);
    let mut reached: Vec<i64> = facts
        .iter()
        .map(|f| f.period)
        .chain(carry.period_k.keys().copied())
        .chain(3..=status_period.min(5))
        .filter(|p| (3..=5).contains(p))
        .collect();
    reached.sort_unstable();
    reached.dedup();
    let periods = [1, 2]
        .into_iter()
        .chain(reached)
        .map(|p| Period {
            number: p,
            s0: period_start_s(p).unwrap_or(0),
            t_image: carry.period_k.get(&p).copied(),
        })
        .collect();
    let match_id = id_of(header.get("id"))
        .or_else(|| id_of(competition.get("id")))
        .unwrap_or_default();
    MatchPayload {
        id: format!("espn-{match_id}"),
        competition: ctx.competition.into(),
        channel: ctx.channel.into(),
        state,
        teams,
        players,
        periods,
        facts,
        possession,
        delay_s: (ctx.delay_s * 10.0).round() / 10.0,
        updated_ms: now_ms,
    }
}
