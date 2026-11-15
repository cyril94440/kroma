use std::collections::BTreeMap;

use serde_json::Value;

use super::state::Sweep;
use super::Sport;
use crate::espn::scoreboard;
use crate::sport::{cadence_seconds, days_to_read, from_espn, OTHER_PATHS};

const FAR_TTL_S: f64 = 1800.0;
const FORCE_GAP_S: f64 = 20.0;

impl Sport {
    pub(crate) fn step_sweep(&self, now: f64) -> bool {
        let settings = self.settings();
        let watched = self.watched();
        let next = {
            let mut st = self.lock();
            if st.events.sweep.is_none() {
                let forced =
                    std::mem::take(&mut st.events.force) && now - st.events.read_at >= FORCE_GAP_S;
                let cadence = cadence_seconds(&st.events.list, watched, now);
                if !forced && now - st.events.read_at < cadence {
                    return false;
                }
                let far = now - st.events.far_at > FAR_TTL_S;
                st.events.sweep = Some(build_sweep(&settings, now, far));
            }
            st.events.sweep.as_mut().and_then(|s| s.queue.pop())
        };
        let Some((key, path)) = next else {
            self.finish_sweep(now);
            return false;
        };
        let fetched = self.espn.get(&path);
        let mut st = self.lock();
        if let Err(e) = &fetched {
            tracing::debug!(error = %e, key, "scoreboard not read");
        }
        if let Some(sweep) = st.events.sweep.as_mut() {
            sweep.responses.insert(key, fetched.ok());
        }
        true
    }

    fn finish_sweep(&self, now: f64) {
        let mut st = self.lock();
        let Some(sweep) = st.events.sweep.take() else {
            return;
        };
        if !sweep.responses.values().any(Option::is_some) {
            st.events.read_at = now;
            return;
        }
        let near: BTreeMap<String, Value> = sweep
            .responses
            .iter()
            .filter_map(|(k, v)| v.clone().map(|v| (k.clone(), v)))
            .collect();
        if sweep.far_included {
            st.events.far = near
                .iter()
                .filter(|(k, _)| is_far(k, now))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            st.events.far_at = now;
        }
        let mut merged = st.events.far.clone();
        merged.extend(near);
        st.events.list = from_espn(&merged, now);
        st.events.read_at = now;
        let events = st.events.list.clone();
        st.memory.expire(&events, now);
        self.store.kv_put(super::state::MEMORY_KEY, &st.memory);
        self.trace_events(&events);
    }
}

fn is_far(key: &str, now: f64) -> bool {
    let (near, _) = days_to_read(now);
    key.split(':')
        .nth(2)
        .is_some_and(|day| !near.iter().any(|n| n == day))
}

fn build_sweep(settings: &crate::settings::Settings, now: f64, far: bool) -> Sweep {
    let (near, ahead) = days_to_read(now);
    let days: Vec<String> = near
        .into_iter()
        .chain(if far { ahead } else { Vec::new() })
        .collect();
    let mut queue: Vec<(String, String)> = Vec::new();
    for (code, _, _) in crate::sport::FOOT
        .iter()
        .filter(|(c, _, _)| settings.competitions.iter().any(|x| x == c))
    {
        for day in &days {
            queue.push((format!("foot:{code}:{day}"), scoreboard(code, day)));
        }
    }
    for (key, path) in OTHER_PATHS {
        let on = match key {
            "tennis:atp" | "tennis:wta" => settings.tennis,
            "f1" => settings.formula_one,
            _ => settings.golf,
        };
        if on {
            queue.push((key.to_string(), path.to_string()));
        }
    }
    queue.reverse();
    Sweep {
        queue,
        responses: BTreeMap::new(),
        far_included: far,
    }
}
