use serde_json::{json, Value};

use super::state::FOLLOW_KEY;
use super::Sport;
use crate::prono::{clock_seconds, measured_delay, Chrono, Status};

const CHRONOS_KEPT: usize = 30;
const EDGE_LATENCY_S: f64 = 3.0;
const EARLY_S: i64 = 600;
const LATE_S: i64 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    NoMatch,
    NotInPlay,
    Unreadable,
    Implausible,
}

impl CalibrationError {
    pub fn code(self) -> &'static str {
        match self {
            CalibrationError::NoMatch => "noMatch",
            CalibrationError::NotInPlay => "notInPlay",
            CalibrationError::Unreadable => "unreadable",
            CalibrationError::Implausible => "implausible",
        }
    }
}

impl Sport {
    /// A reading of the match chrono as shown on a picture: `captured_ms` is the server time of the frame read (its programme date time), else now minus the live edge. It anchors the period on the image of the stream and, with ESPN's minute changes, measures the picture's delay.
    pub fn calibrate(
        &self,
        clock_text: &str,
        captured_ms: Option<i64>,
    ) -> Result<Value, CalibrationError> {
        let now = self.now();
        let mut st = self.lock();
        let Some(target) = st.follow.target.clone() else {
            return Err(CalibrationError::NoMatch);
        };
        let (Some(espn_clock), true) = (
            st.follow.espn_clock_s,
            st.follow.espn_state == "in" && !st.follow.half_time,
        ) else {
            return Err(CalibrationError::NotInPlay);
        };
        let Some(chrono) = clock_seconds(clock_text) else {
            return Err(CalibrationError::Unreadable);
        };
        if chrono < espn_clock - EARLY_S || chrono > espn_clock + LATE_S + 59 {
            drop(st);
            self.trace("chrono", "chrono_incoherent", json!({"match": target, "lu": clock_text, "chrono_s": chrono, "espn_s": espn_clock}));
            return Err(CalibrationError::Implausible);
        }
        let ts = captured_ms.map_or(now - EDGE_LATENCY_S, |ms| ms as f64 / 1000.0);
        let regime = st.pronos.direct_key.clone();
        let period = st.follow.espn_period.max(1);
        let reading = Chrono {
            ts_s: ts,
            chrono_s: chrono as f64,
            period,
            regime,
        };
        if !st
            .follow
            .saved
            .chronos
            .iter()
            .any(|c| (c.ts_s - ts).abs() < 1.0)
        {
            st.follow.saved.chronos.push(reading.clone());
            let excess = st.follow.saved.chronos.len().saturating_sub(CHRONOS_KEPT);
            st.follow.saved.chronos.drain(..excess);
        }
        let measured = st
            .follow
            .change
            .map(|(change_at, clock_s)| measured_delay(ts, chrono as f64, change_at, clock_s));
        let retained = measured.is_some_and(|delay| {
            st.pronos
                .current
                .as_ref()
                .is_some_and(|c| c.status == Status::Running)
                && st.pronos.measure(&target, delay, now)
        });
        st.follow.last_fetch = 0.0;
        self.store.kv_put(FOLLOW_KEY, &st.follow.saved);
        let (delay, source) = st.pronos.delay(now);
        drop(st);
        self.save_pronos();
        self.trace("chrono", "calibrage", json!({"match": target, "lu": clock_text, "chrono_s": chrono, "periode": period, "ts_image": ts, "retard_mesure_s": measured, "mesure_retenue": retained, "delai": {"s": delay, "source": source}}));
        Ok(
            json!({"retained": true, "periodAnchored": period, "measuredDelayS": measured, "measureKept": retained, "delayS": delay, "delaySource": source}),
        )
    }
}
