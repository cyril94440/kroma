use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::state::FOLLOW_KEY;
use super::Sport;
use crate::espn::summary;
use crate::live_match::{extract, period_k, period_start_s, Carry, Context, MatchPayload};
use crate::prono::{Chrono, DirectKey, Status};

const AFTER_POST_S: f64 = 120.0;
const SPREAD_MAX_MS: f64 = 30_000.0;
const SEEN_KEEP_MS: i64 = 6 * 3600 * 1000;

/// K (ms) of each period from the chrono readings: the median after rejecting readings further than 30 s from it. The period in play only takes readings of the current regime of the live (a switch or a restart shifts the picture).
pub fn period_ks(
    chronos: &[Chrono],
    current_period: Option<i64>,
    regime: Option<&DirectKey>,
) -> BTreeMap<i64, i64> {
    let mut by_period: BTreeMap<i64, Vec<(f64, f64)>> = BTreeMap::new();
    for reading in chronos {
        if Some(reading.period) == current_period
            && (regime.is_none() || reading.regime.as_ref() != regime)
        {
            by_period.entry(reading.period).or_default();
            continue;
        }
        by_period.entry(reading.period).or_default().push((
            reading.ts_s,
            reading.chrono_s - period_start_s(reading.period).unwrap_or(2700) as f64,
        ));
    }
    by_period
        .into_iter()
        .filter_map(|(period, readings)| {
            let values: Vec<f64> = readings
                .iter()
                .map(|(ts, chrono)| ts * 1000.0 - chrono * 1000.0)
                .collect();
            let median = crate::prono::median(&values)?;
            let kept: Vec<(f64, f64)> = readings
                .iter()
                .zip(&values)
                .filter(|(_, v)| (**v - median).abs() <= SPREAD_MAX_MS)
                .map(|(r, _)| *r)
                .collect();
            period_k(if kept.is_empty() { &readings } else { &kept }).map(|k| (period, k as i64))
        })
        .collect()
}

impl Sport {
    pub(crate) fn step_follow(&self, now: f64) -> bool {
        let settings = self.settings();
        let watched = self.watched();
        let target = {
            let mut st = self.lock();
            let from_prono = st
                .pronos
                .current
                .as_ref()
                .filter(|c| c.status != Status::Final && c.id.starts_with("espn-"))
                .map(|c| c.id.clone());
            let target = st
                .detect
                .matched
                .as_ref()
                .map(|m| m.found.id.clone())
                .or(from_prono.clone());
            let active = watched || from_prono.is_some();
            let expired = st.follow.post_since.is_some_and(|t| now - t > AFTER_POST_S);
            if target.is_none() || !active || expired {
                if st.follow.published {
                    st.follow.published = false;
                    st.follow.payload = None;
                    st.follow.target = None;
                    st.follow.post_since = None;
                    self.emit_if_changed(
                        &mut st,
                        "sport.match",
                        json!({"match": null}),
                        "null".into(),
                    );
                }
                return false;
            }
            let target = target.unwrap();
            if st.follow.target.as_ref() != Some(&target) {
                st.follow.target = Some(target.clone());
                st.follow.post_since = None;
                st.follow.last_fetch = 0.0;
                if st.follow.saved.match_id != target {
                    st.follow.saved = Default::default();
                    st.follow.saved.match_id = target.clone();
                }
            }
            if now - st.follow.last_fetch < settings.match_poll_s {
                return false;
            }
            st.follow.last_fetch = now;
            target
        };
        let fetched = self
            .espn
            .get(&summary("all", target.trim_start_matches("espn-")));
        match fetched {
            Ok(payload) => self.apply_summary(payload, now),
            Err(e) => self.trace(
                "espn",
                "summary_indisponible",
                json!({"match": target, "erreur": e.to_string()}),
            ),
        }
        true
    }

    fn apply_summary(&self, summary_json: Value, now: f64) {
        let settings = self.settings();
        let mut st = self.lock();
        let (delay, _) = st.pronos.delay(now);
        let delay_s = delay.max(settings.spoiler_delay_s);
        let regime = st.pronos.direct_key.clone();
        let status = summary_json
            .get("header")
            .and_then(|h| h.get("competitions"))
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("status"))
            .cloned()
            .unwrap_or(Value::Null);
        let espn_period = status.get("period").and_then(Value::as_i64).unwrap_or(0);
        let ks = period_ks(
            &st.follow.saved.chronos,
            Some(espn_period).filter(|p| *p > 0),
            regime.as_ref(),
        );
        let competition = st
            .detect
            .matched
            .as_ref()
            .map(|m| m.found.competition.clone())
            .unwrap_or_default();
        let channel = st
            .presence
            .channel
            .as_ref()
            .map(|c| c.name.clone())
            .unwrap_or_default();
        let previous = st
            .follow
            .payload
            .as_ref()
            .map(|p| p.possession.clone())
            .unwrap_or_default();
        let mut seen = std::mem::take(&mut st.follow.saved.seen);
        let ctx = Context {
            delay_s,
            now_s: now,
            channel: &channel,
            competition: &competition,
        };
        let payload = extract(
            &summary_json,
            &mut Carry {
                seen: &mut seen,
                period_k: &ks,
                possession: &previous,
            },
            &ctx,
        );
        let horizon = payload.updated_ms - SEEN_KEEP_MS;
        seen.retain(|_, at| *at >= horizon);
        st.follow.saved.seen = seen;
        self.note_espn_clock(&mut st, &status, now);
        if payload.state == "post" && st.follow.post_since.is_none() {
            st.follow.post_since = Some(now);
        }
        self.store.kv_put(FOLLOW_KEY, &st.follow.saved);
        st.follow.summary = summary_json.clone();
        st.follow.published = true;
        let fingerprint = fingerprint(&payload);
        st.follow.payload = Some(payload.clone());
        self.emit_if_changed(
            &mut st,
            "sport.match",
            json!({"match": payload}),
            fingerprint,
        );
        drop(st);
        self.announce_goals(&payload, &summary_json, now);
    }

    fn note_espn_clock(&self, st: &mut super::State, status: &Value, now: f64) {
        let display = status
            .get("displayClock")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let kind = status.get("type").cloned().unwrap_or(Value::Null);
        st.follow.espn_state = kind
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        st.follow.espn_period = status.get("period").and_then(Value::as_i64).unwrap_or(0);
        st.follow.half_time = kind
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|n| n.eq_ignore_ascii_case("STATUS_HALFTIME"));
        let minute: Option<i64> = (!display.contains('+'))
            .then(|| {
                display
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .flatten();
        st.follow.espn_clock_s = minute.map(|m| m * 60);
        if let Some(n) = minute.filter(|_| st.follow.espn_state == "in" && !st.follow.half_time) {
            if let Some((previous, at)) = st.follow.minute {
                if previous != n {
                    st.follow.change = Some(((at + now) / 2.0, (n * 60) as f64));
                }
            }
            st.follow.minute = Some((n, now));
        }
    }
}

fn fingerprint(payload: &MatchPayload) -> String {
    let mut copy = payload.clone();
    copy.updated_ms = 0;
    serde_json::to_string(&copy).unwrap_or_default()
}
