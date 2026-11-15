use super::Sport;
use crate::detect::{flat_matches, FlatMatch};
use crate::espn::scoreboard;
use crate::iso::day_key;

const BOARD_TTL_S: f64 = 60.0;

impl Sport {
    pub(crate) fn step_board(&self, now: f64) -> BoardStep {
        let days = [day_key(now), day_key(now - 24.0 * 3600.0)];
        let stale = {
            let st = self.lock();
            days.iter()
                .find(|d| {
                    st.board
                        .days
                        .get(*d)
                        .is_none_or(|(at, _)| now - at > BOARD_TTL_S)
                })
                .cloned()
        };
        let Some(day) = stale else {
            return BoardStep::Ready(self.board_matches(&days));
        };
        let fetched = self.espn.get(&scoreboard("all", &day));
        let mut st = self.lock();
        match fetched {
            Ok(payload) => {
                st.board
                    .days
                    .insert(day, (now, flat_matches(&payload, "all")));
            }
            Err(e) => {
                tracing::debug!(error = %e, "scoreboard not read");
                st.board
                    .days
                    .entry(day)
                    .or_insert((now - BOARD_TTL_S + 10.0, Vec::new()))
                    .0 = now - BOARD_TTL_S + 10.0;
                return BoardStep::Failed;
            }
        }
        BoardStep::Fetched
    }

    pub(crate) fn board_matches(&self, days: &[String]) -> Vec<FlatMatch> {
        let st = self.lock();
        let lists: Vec<Vec<FlatMatch>> = days
            .iter()
            .rev()
            .filter_map(|d| st.board.days.get(d).map(|(_, m)| m.clone()))
            .collect();
        crate::detect::merge(&lists)
    }
}

pub(crate) enum BoardStep {
    Ready(Vec<FlatMatch>),
    Fetched,
    Failed,
}
