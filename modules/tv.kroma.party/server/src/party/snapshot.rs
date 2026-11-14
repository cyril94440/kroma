use serde_json::{json, Map, Value};

use super::{Actor, Party, ViewerStats};
use crate::chat_rules::{HISTORY_MAX, RETENTION_MS};

const STATS_FRESH_MS: i64 = 30_000;

fn stats_view(s: &ViewerStats) -> Value {
    json!({
        "name": s.name, "bufferS": s.buffer, "bandwidthKbps": s.bandwidth_kbps, "stalls": s.stalls,
        "rebufferMs": s.rebuffer_ms, "dropped": s.dropped, "resolution": s.resolution,
        "playing": s.playing, "delayS": s.delay, "paused": s.paused, "at": s.at,
    })
}

impl Party {
    pub fn snapshot(&self, actor: &Actor) -> Value {
        let presence = self.presence();
        let now = self.now();
        let (seq, vote, poll, last_poll, activity, sync, stats) = {
            let st = self.lock();
            let stats: Map<String, Value> = st
                .stats
                .iter()
                .filter(|(id, s)| now - s.at < STATS_FRESH_MS && presence.iter().any(|v| &v.user_id == *id))
                .map(|(id, s)| (id.clone(), stats_view(s)))
                .collect();
            (
                st.seq,
                st.vote.as_ref().map(|v| v.view(now)),
                st.poll.as_ref().map(|p| p.view(now)),
                st.last_poll.clone(),
                st.activity.as_ref().map(|(label, _)| label.clone()),
                st.sync.peek(now),
                stats,
            )
        };
        let messages = self
            .store
            .recent_messages(HISTORY_MAX as u32, None, Some(now - RETENTION_MS))
            .unwrap_or_default();
        json!({
            "seq": seq,
            "now": now,
            "me": { "userId": actor.person.user_id, "name": actor.person.name, "admin": actor.admin },
            "messages": messages,
            "vote": vote,
            "poll": poll,
            "lastPoll": last_poll,
            "activity": activity,
            "sync": sync,
            "presence": presence,
            "stats": stats,
            "config": { "voteSeconds": self.vote_seconds(), "agentName": self.agent_name() },
        })
    }

    pub fn record_stats(&self, actor: &Actor, mut s: ViewerStats) {
        s.at = self.now();
        s.name = actor.person.name.clone();
        self.lock().stats.insert(actor.person.user_id.clone(), s);
    }
}
