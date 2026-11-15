use serde_json::{json, Value};

use super::state::ANNOUNCED_KEY;
use super::Sport;
use crate::cards::validate;
use crate::clips::real_times;
use crate::live_match::{revealed, Fact, MatchPayload};
use crate::strings;

const LATE_S: f64 = 120.0;
const KEPT: usize = 400;

fn score_at(m: &MatchPayload, goal: &Fact) -> String {
    let (mut dom, mut ext) = (0, 0);
    let mut goals: Vec<&Fact> = m.facts.iter().filter(|f| f.kind == "but").collect();
    goals.sort_by_key(|f| (f.period, f.s));
    for g in goals {
        match g.side.as_str() {
            "dom" => dom += 1,
            "ext" => ext += 1,
            _ => {}
        }
        if g.id == goal.id {
            break;
        }
    }
    format!("{dom} - {ext}")
}

fn reveal_ms(goal: &Fact, delay_s: f64) -> f64 {
    goal.t_image
        .map_or(goal.t_vu as f64 + delay_s * 1000.0, |t| t as f64)
}

impl Sport {
    pub(crate) fn announce_goals(&self, m: &MatchPayload, summary: &Value, now: f64) {
        let real = real_times(summary);
        let settings = self.settings();
        let now_ms = now * 1000.0;
        let due: Vec<Fact> = {
            let st = self.lock();
            m.facts
                .iter()
                .filter(|f| {
                    f.kind == "but"
                        && !st.announced.contains(&format!("{}:{}", m.id, f.id))
                        && revealed(
                            f.t_image,
                            f.t_vu,
                            now_ms - settings.reveal_margin_s * 1000.0,
                            m.delay_s,
                        )
                })
                .cloned()
                .collect()
        };
        for goal in due {
            let espn_reveal = real.get(&goal.id).map(|t| (t + m.delay_s) * 1000.0);
            let late = now_ms
                - espn_reveal.map_or(reveal_ms(&goal, m.delay_s), |e| {
                    e.min(reveal_ms(&goal, m.delay_s))
                })
                > LATE_S * 1000.0;
            if settings.goal_announcements && !late {
                self.say_goal(m, &goal, &settings.language);
            }
            let mut st = self.lock();
            st.announced.insert(format!("{}:{}", m.id, goal.id));
            if st.announced.len() > KEPT {
                let drop: Vec<String> = st
                    .announced
                    .iter()
                    .take(st.announced.len() - KEPT)
                    .cloned()
                    .collect();
                drop.iter().for_each(|k| {
                    st.announced.remove(k);
                });
            }
            self.store.kv_put(ANNOUNCED_KEY, &st.announced);
        }
    }

    fn say_goal(&self, m: &MatchPayload, goal: &Fact, language: &str) {
        let score = score_at(m, goal);
        let scorer = goal
            .player
            .as_ref()
            .and_then(|id| m.players.iter().find(|p| &p.id == id))
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let vars = [
            ("home", m.teams.dom.name.as_str()),
            ("away", m.teams.ext.name.as_str()),
            ("score", score.as_str()),
            ("minute", goal.minute.as_str()),
            ("scorer", scorer.as_str()),
        ];
        let text = strings::text(
            language,
            if scorer.is_empty() {
                "sport.announce.goalNoScorer"
            } else {
                "sport.announce.goal"
            },
            &vars,
        )
        .trim()
        .to_string();
        let card = validate(
            &json!({"kind": "match", "home": m.teams.dom.name, "away": m.teams.ext.name, "competition": m.competition, "score": score, "status": "live", "channel": Some(m.channel.clone()).filter(|c| !c.is_empty())}),
        );
        self.speak(&text, card, "but");
    }

    pub(crate) fn speak(&self, text: &str, card: Option<Value>, kind: &str) {
        let outcome = self.room.say(text, card);
        self.trace("audit", "annonce", json!({"genre": kind, "texte": text, "envoye": outcome.is_ok(), "erreur": outcome.err().map(|e| e.to_string())}));
    }
}
