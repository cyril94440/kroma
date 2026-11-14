use serde_json::{json, Value};

use super::Party;
use crate::store::LogEntry;

impl Party {
    pub fn recent_votes(&self, limit: u32) -> Vec<Value> {
        self.store.recent_votes(limit.clamp(1, 200)).unwrap_or_default()
    }

    pub fn ingest_logs(&self, entries: &[LogEntry]) {
        if let Err(e) = self.store.insert_logs(entries) {
            tracing::error!(error = %e, "client log not stored");
        }
    }

    pub fn read_logs(&self, minutes: i64, user_id: Option<&str>, category: Option<&str>, limit: u32) -> Vec<LogEntry> {
        let since = self.now() - minutes.clamp(1, 4320) * 60_000;
        self.store.logs(since, user_id, category, limit.clamp(1, 500)).unwrap_or_default()
    }

    pub fn health(&self) -> Value {
        let now = self.now();
        let st = self.lock();
        json!({
            "ok": now - st.last_tick < 30_000,
            "tickAgoS": (now - st.last_tick) / 1000,
            "seq": st.seq,
            "voteOpen": st.vote.is_some(),
            "pollOpen": st.poll.is_some(),
            "partyMembers": st.sync.member_count(),
        })
    }
}
