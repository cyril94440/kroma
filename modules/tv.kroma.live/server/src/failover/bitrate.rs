use std::collections::VecDeque;

pub const MIN_SAMPLES: usize = 3;
pub const WINDOW: usize = 30;

#[derive(Debug, Default)]
pub struct BitrateWatch {
    key: Option<String>,
    history: VecDeque<f64>,
}

fn median_of(values: &VecDeque<f64>) -> f64 {
    let mut sorted: Vec<f64> = values.iter().copied().collect();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 { sorted[middle] } else { (sorted[middle - 1] + sorted[middle]) / 2.0 }
}

impl BitrateWatch {
    pub fn median(&self) -> Option<f64> {
        (self.history.len() >= MIN_SAMPLES).then(|| median_of(&self.history))
    }

    pub fn flowing(&mut self, running: bool, kbps: f64, key: Option<&str>, healthy: bool, min_kbps: f64) -> bool {
        if self.key.as_deref() != key {
            self.key = key.map(str::to_string);
            self.history.clear();
        }
        let kbps = if kbps.is_finite() { kbps.max(0.0) } else { 0.0 };
        let usual = self.median();
        if !running {
            return false;
        }
        let alive = kbps > min_kbps || usual.is_some_and(|usual| usual <= min_kbps && kbps > 0.0 && kbps >= usual / 2.0);
        let learns = if self.history.len() >= MIN_SAMPLES { alive } else { kbps > min_kbps || healthy };
        if kbps > 0.0 && learns {
            if self.history.len() >= WINDOW {
                self.history.pop_front();
            }
            self.history.push_back(kbps);
        }
        alive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: f64 = 100.0;

    #[test]
    fn a_low_rate_channel_is_alive_once_its_usual_rate_is_known() {
        let mut watch = BitrateWatch::default();
        let radio = Some("FR: FRANCE INFO");

        let learning: Vec<bool> = (0..3).map(|_| watch.flowing(true, 64.0, radio, true, MIN)).collect();

        assert_eq!(learning, [false; 3]);
        assert!(watch.flowing(true, 64.0, radio, true, MIN));
        assert!(!watch.flowing(true, 20.0, radio, true, MIN));
        assert!(!watch.flowing(true, 0.0, radio, true, MIN));
        assert!(!watch.flowing(false, 64.0, radio, true, MIN));
    }

    #[test]
    fn a_high_rate_channel_that_collapses_stays_dead_and_the_outage_is_not_learned() {
        let mut watch = BitrateWatch::default();
        let tv = Some("FR: TF1");
        for _ in 0..5 {
            assert!(watch.flowing(true, 4000.0, tv, true, MIN));
        }

        let collapsed: Vec<bool> = (0..11).map(|_| watch.flowing(true, 80.0, tv, true, MIN)).collect();

        assert_eq!(collapsed, [false; 11]);
        assert_eq!(watch.median(), Some(4000.0));
        assert!(watch.flowing(true, 101.0, tv, true, MIN));
        assert!(!watch.flowing(true, 100.0, tv, true, MIN));
    }

    #[test]
    fn the_history_starts_again_when_the_channel_changes() {
        let mut watch = BitrateWatch::default();
        for _ in 0..4 {
            watch.flowing(true, 64.0, Some("FR: RADIO"), true, MIN);
        }
        assert!(watch.median().is_some());

        watch.flowing(true, 64.0, Some("FR: AUTRE"), true, MIN);

        assert_eq!(watch.median(), None);
    }

    #[test]
    fn a_low_rate_without_a_healthy_picture_is_never_learned() {
        let mut watch = BitrateWatch::default();

        let unhealthy: Vec<bool> = (0..8).map(|_| watch.flowing(true, 80.0, Some("FR: TF1"), false, MIN)).collect();

        assert_eq!(unhealthy, [false; 8]);
        assert_eq!(watch.median(), None);
        let mut healthy = BitrateWatch::default();
        let radio: Vec<bool> = (0..5).map(|_| healthy.flowing(true, 64.0, Some("FR: RADIO"), true, MIN)).collect();
        assert_eq!(radio, [false, false, false, true, true]);
    }

    #[test]
    fn the_window_keeps_the_last_thirty_samples() {
        let mut watch = BitrateWatch::default();
        for _ in 0..30 {
            watch.flowing(true, 200.0, Some("a"), true, MIN);
        }
        for _ in 0..30 {
            watch.flowing(true, 400.0, Some("a"), true, MIN);
        }

        assert_eq!(watch.median(), Some(400.0));
    }
}
