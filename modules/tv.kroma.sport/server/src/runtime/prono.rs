use serde_json::{json, Value};

use super::state::PRONO_KEY;
use super::Sport;
use crate::cards::validate;
use crate::detect::{flat_matches, FlatMatch};
use crate::espn::scoreboard;
use crate::iso::day_key;
use crate::prono::{abandoned_text, final_text, find_for_prono, Effect, FinalText, PRONO_TICK_S};
use crate::strings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoteError {
    NoProno,
    Closed,
    Invalid,
}

impl VoteError {
    pub fn code(self) -> &'static str {
        match self {
            VoteError::NoProno => "noOpenProno",
            VoteError::Closed => "closed",
            VoteError::Invalid => "invalidScore",
        }
    }
}

fn describe(reason: &str) -> String {
    match reason {
        "matchIntrouvable" => "match introuvable dans les ligues suivies (ESPN)",
        "plusieursMatchs" => "plusieurs matchs correspondent : précise les équipes",
        "dejaJoue" => "un prono a déjà eu lieu pour ce match",
        "tropAvance" => "match terminé ou déjà trop avancé (prono ouvert jusqu'à la 10e minute)",
        "tropLointain" => "coup d'envoi trop lointain",
        other => other,
    }
    .to_string()
}

impl Sport {
    pub(crate) fn step_prono(&self, now: f64) -> bool {
        let settings = self.settings();
        let active = self.lock().pronos.active();
        if !settings.prono || (!active && !self.watched()) {
            self.lock().pronos.purge(now);
            return false;
        }
        if now - self.lock().prono_at < PRONO_TICK_S {
            return false;
        }
        let matches = match self.step_board(now) {
            super::board::BoardStep::Fetched => return true,
            super::board::BoardStep::Failed => None,
            super::board::BoardStep::Ready(m) => Some(m),
        };
        let effects = {
            let mut st = self.lock();
            st.prono_at = now;
            let (delay, _) = st.pronos.delay(now);
            let lagged = st
                .lag
                .read("all", matches, now, delay, settings.spoiler_delay_s);
            let channel = st
                .presence
                .channel
                .clone()
                .map(|c| c.name)
                .unwrap_or_default();
            let required: Vec<(String, String)> = st
                .presence
                .viewers
                .iter()
                .map(|v| (v.user_id.clone(), v.name.clone()))
                .collect();
            let detected = st
                .detect
                .matched
                .as_ref()
                .filter(|m| {
                    st.presence
                        .channel
                        .as_ref()
                        .is_some_and(|c| c.id == m.channel_id)
                })
                .map(|m| lagged_view(&m.found, lagged.as_deref()));
            st.pronos.cycle(
                lagged.as_deref(),
                detected.as_ref(),
                &channel,
                &required,
                now,
            )
        };
        if !effects.is_empty() {
            self.trace(
                "audit",
                "prono",
                json!({"effets": effects.iter().map(|e| format!("{e:?}")).collect::<Vec<_>>()}),
            );
            self.after_effects(&effects);
        }
        self.save_pronos();
        true
    }

    fn after_effects(&self, effects: &[Effect]) {
        let language = self.settings().language;
        let (current, cancelled) = {
            let st = self.lock();
            (st.pronos.current.clone(), st.pronos.cancelled.clone())
        };
        for effect in effects {
            match (effect, &current, &cancelled) {
                (Effect::Opened, Some(c), _) => {
                    let text = strings::text(
                        &language,
                        "sport.announce.prono.opened",
                        &[("home", &c.home), ("away", &c.away)],
                    );
                    let card = validate(
                        &json!({"kind": "match", "home": c.home, "away": c.away, "competition": c.competition, "status": "upcoming", "start": c.kickoff as i64}),
                    );
                    self.speak(&text, card, "prono");
                }
                (Effect::Finished, Some(c), _) => {
                    if let Some(text) = final_text(c) {
                        self.speak(&render(&language, &text), None, "prono");
                    }
                }
                (Effect::Abandoned | Effect::Away, _, Some(c))
                    if matches!(effect, Effect::Abandoned) =>
                {
                    self.speak(&render(&language, &abandoned_text(c)), None, "prono");
                }
                _ => {}
            }
        }
    }

    pub(crate) fn save_pronos(&self) {
        let mut st = self.lock();
        if std::mem::take(&mut st.pronos.dirty) {
            self.store.kv_put(PRONO_KEY, &st.pronos.to_file());
        }
    }

    pub fn vote(
        &self,
        user_id: &str,
        name: &str,
        match_id: &str,
        score: &str,
    ) -> Result<(), VoteError> {
        let now = self.now();
        let outcome = self.lock().pronos.vote(user_id, name, match_id, score, now);
        self.save_pronos();
        match outcome {
            (true, _) => Ok(()),
            (false, "noOpenProno") => Err(VoteError::NoProno),
            (false, "closed") => Err(VoteError::Closed),
            _ => Err(VoteError::Invalid),
        }
    }

    /// Starts a prediction on request (the `start_prono` tool): the same windows and checks as the automatic detection.
    pub fn start_prono(&self, home: &str, away: &str) -> Result<Value, String> {
        let now = self.now();
        if self.lock().pronos.active() {
            return Err("un prono est déjà en cours".into());
        }
        let matches = self
            .fetch_board_now(now)
            .ok_or("source des matchs injoignable : réessaie plus tard")?;
        let (found, required, channel) = {
            let st = self.lock();
            let found = find_for_prono(home, away, &matches, now, &st.pronos)
                .map_err(describe)?
                .clone();
            let required: Vec<(String, String)> = st
                .presence
                .viewers
                .iter()
                .map(|v| (v.user_id.clone(), v.name.clone()))
                .collect();
            (
                found,
                required,
                st.presence
                    .channel
                    .clone()
                    .map(|c| c.name)
                    .unwrap_or_default(),
            )
        };
        if !self
            .lock()
            .pronos
            .open(&found, &channel, &required, false, now)
        {
            return Err("ouverture refusée".into());
        }
        self.after_effects(&[Effect::Opened]);
        self.save_pronos();
        self.trace(
            "audit",
            "start_prono",
            json!({"home": home, "away": away, "match": found.label()}),
        );
        Ok(
            json!({"ok": true, "match": found.label(), "note": "le système a déjà annoncé le prono dans le chat : une phrase d'ambiance suffit"}),
        )
    }

    pub(crate) fn fetch_board_now(&self, now: f64) -> Option<Vec<FlatMatch>> {
        let days = [day_key(now), day_key(now - 24.0 * 3600.0)];
        let mut lists = Vec::new();
        for day in &days {
            let payload = self.espn.get(&scoreboard("all", day)).ok()?;
            let list = flat_matches(&payload, "all");
            self.lock()
                .board
                .days
                .insert(day.clone(), (now, list.clone()));
            lists.push(list);
        }
        lists.reverse();
        Some(crate::detect::merge(&lists))
    }

    pub fn prono_payload(&self) -> Option<Value> {
        let now = self.now();
        self.lock().pronos.payload(now)
    }
}

fn render(language: &str, text: &FinalText) -> String {
    let vars: Vec<(&str, &str)> = text.vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
    strings::text(language, text.key, &vars)
}

fn lagged_view(fresh: &FlatMatch, lagged: Option<&[FlatMatch]>) -> FlatMatch {
    let mut view = fresh.clone();
    match lagged.and_then(|l| l.iter().find(|m| m.id == fresh.id)) {
        Some(old) => {
            view.score = old.score.clone();
            view.minute = old.minute.clone();
            view.state = old.state.clone();
            view.period = old.period;
            view.minute_n = old.minute_n;
        }
        None => {
            view.score = "0-0".into();
            view.minute = String::new();
            view.minute_n = None;
        }
    }
    view
}
