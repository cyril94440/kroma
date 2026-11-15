use super::{
    round2, Cause, Class, Episode, State, StreamHealth, CATCH_UP_MARGIN, CLASSIFY_BLOCKS, DEGRADED_LOSS, EXIT_BLOCKS, EXIT_LOSS,
    FROZEN_THRESHOLD, LOSS_WINDOW, MAX_EPISODES, SEVERE_LOSS,
};

impl StreamHealth {
    pub(super) fn classify_pending(&mut self, lag: f64) {
        let blocks = self.blocks;
        let mut remaining = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        for mut gap in pending {
            gap.lowest_after = gap.lowest_after.min(lag);
            if blocks - gap.closed_at < CLASSIFY_BLOCKS {
                remaining.push(gap);
                continue;
            }
            let (class, magnitude) = if gap.lowest_after - gap.opening_lag <= CATCH_UP_MARGIN {
                (Class::CaughtUp, 0.0)
            } else {
                let lost = round2((gap.closing_lag - gap.opening_lag).max(0.0));
                self.channel_loss = round2(self.channel_loss + lost);
                (Class::Lost, lost)
            };
            if self.episodes.len() >= MAX_EPISODES {
                self.episodes.pop_front();
            }
            self.episodes.push_back(Episode {
                at: gap.closed_at,
                class,
                magnitude,
                freeze: round2((gap.peak - gap.opening_lag).max(0.0)),
            });
            self.episodes_total += 1;
        }
        self.pending = remaining;
    }

    pub fn cause(&self) -> Cause {
        match (self.source_hints > 0, self.local_hints > 0) {
            (true, false) => Cause::Source,
            (false, true) => Cause::Local,
            _ => Cause::Undetermined,
        }
    }

    pub(super) fn update_frozen(&mut self, lag: f64) {
        if !self.frozen {
            if self.lag_ago(3).is_some_and(|before| lag - before >= FROZEN_THRESHOLD) {
                self.frozen = true;
                self.frozen_since = Some(self.blocks);
            }
        } else if self.lag_ago(2).is_some_and(|before| (lag - before).abs() <= 0.3) {
            self.frozen = false;
            self.frozen_since = None;
        }
    }

    pub fn recent_loss(&self) -> f64 {
        round2(
            self.episodes
                .iter()
                .filter(|e| e.class == Class::Lost && self.blocks.saturating_sub(e.at) <= LOSS_WINDOW)
                .map(|e| e.magnitude)
                .sum::<f64>()
                + 0.0,
        )
    }

    pub(super) fn update_state(&mut self) {
        if matches!(self.state, State::Starting | State::Undetermined) && self.anchor.is_none() {
            return;
        }
        let loss = self.recent_loss();
        let long_freeze = self.frozen && self.frozen_since.is_some_and(|since| self.blocks - since >= 10);
        if loss >= SEVERE_LOSS || long_freeze {
            self.state = State::Severe;
            self.exit_ok = 0;
        } else if loss >= DEGRADED_LOSS {
            if self.state != State::Severe {
                self.state = State::Degraded;
            }
            self.exit_ok = 0;
        } else if loss <= EXIT_LOSS && matches!(self.state, State::Degraded | State::Severe) {
            self.exit_ok += 1;
            if self.exit_ok >= EXIT_BLOCKS {
                self.state = State::Healthy;
                self.exit_ok = 0;
            }
        } else if self.state == State::Healthy {
            self.exit_ok = 0;
        }
    }
}
