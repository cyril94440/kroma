use serde_json::{json, Value};

use super::Sport;
use crate::sport::SportEvent;

impl Sport {
    pub(crate) fn trace(&self, genre: &str, event: &str, data: Value) {
        self.store.trace(self.now_ms(), genre, event, &data);
    }

    pub(crate) fn trace_events(&self, events: &[SportEvent]) {
        let live: Vec<Value> = events
            .iter()
            .filter(|e| e.state == crate::sport::State::In)
            .map(|e| json!({"id": e.id, "competition": e.competition, "temps": e.clock, "lignes": e.lines}))
            .collect();
        if !live.is_empty() {
            self.trace(
                "espn",
                "scoreboard",
                json!({"direct": live, "total": events.len()}),
            );
        }
    }
}
