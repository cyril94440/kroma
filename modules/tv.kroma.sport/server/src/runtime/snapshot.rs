use serde_json::{json, Value};

use super::Sport;
use crate::sport::{public_view, OnScreen};

impl Sport {
    pub(crate) fn view_value(&self, now: f64) -> Value {
        let st = self.lock();
        let on_screen = OnScreen {
            channel_id: st.presence.channel.as_ref().map(|c| c.id.clone()),
            espn: st
                .detect
                .matched
                .as_ref()
                .map(|m| m.found.id.trim_start_matches("espn-").to_string()),
        };
        serde_json::to_value(public_view(
            &st.events.list,
            &st.memory,
            &st.guide.links,
            &on_screen,
            &st.scan.status,
            now,
        ))
        .unwrap_or(Value::Null)
    }

    /// Everything a client needs to converge: the sequence number it can compare with the events it receives, then each stateful thing.
    pub fn snapshot(&self, user_id: &str, name: &str, admin: bool) -> Value {
        let now = self.now();
        let settings = self.settings();
        let view = self.view_value(now);
        let prono = self.prono_payload();
        let clips = self.clips(None, 300);
        let mut st = self.lock();
        let (delay, source) = st.pronos.delay(now);
        json!({
            "seq": st.seq,
            "now": (now * 1000.0) as i64,
            "me": {"userId": user_id, "name": name, "admin": admin},
            "view": view,
            "match": st.follow.payload,
            "prono": prono,
            "clips": clips,
            "delay": {"seconds": delay, "source": source},
            "channel": st.presence.channel,
            "config": {"goalAnnouncements": settings.goal_announcements, "clips": settings.clips, "prono": settings.prono},
        })
    }

    pub(crate) fn publish_changes(&self, now: f64) {
        let view = self.view_value(now);
        let mut fingerprint = view.clone();
        if let Some(map) = fingerprint.as_object_mut() {
            map.remove("maj");
        }
        let prono = self.prono_payload();
        let prono_print = prono.as_ref().map(|p| {
            let mut copy = p.clone();
            if let Some(map) = copy.as_object_mut() {
                map.remove("ferme_dans_s");
                map.remove("diagnostic");
            }
            copy.to_string()
        });
        let mut st = self.lock();
        self.emit_if_changed(
            &mut st,
            "sport.view",
            json!({"view": view}),
            fingerprint.to_string(),
        );
        self.emit_if_changed(
            &mut st,
            "sport.prono",
            json!({"prono": prono}),
            prono_print.unwrap_or_else(|| "null".into()),
        );
    }
}

impl Sport {
    pub fn sequence(&self) -> u64 {
        self.lock().seq
    }

    /// The season leaderboard, best first, with each person's last known name.
    pub fn leaderboard(&self) -> Value {
        let st = self.lock();
        let mut rows: Vec<Value> = st.pronos.season.iter().map(|(id, s)| json!({"user_id": id, "pseudo": s.name, "points": s.points, "gagnes": s.won})).collect();
        rows.sort_by(|a, b| {
            (
                std::cmp::Reverse(a["points"].as_u64()),
                a["pseudo"].as_str().map(str::to_lowercase),
            )
                .cmp(&(
                    std::cmp::Reverse(b["points"].as_u64()),
                    b["pseudo"].as_str().map(str::to_lowercase),
                ))
        });
        Value::Array(rows)
    }

    pub fn standings_for_page(&self, competition: &str) -> Option<Value> {
        self.standings_cached(
            if competition.is_empty() {
                "Ligue 1"
            } else {
                competition
            },
            self.now(),
        )
    }

    pub fn trace_rows(
        &self,
        genre: Option<&str>,
        before: Option<i64>,
        limit: u32,
    ) -> Vec<crate::store::TraceRow> {
        self.store
            .trace_rows(genre, before, limit)
            .unwrap_or_default()
    }

    /// Forces a fresh read of ESPN, the guide links and the detection at the next ticks.
    pub fn refresh(&self) {
        let mut st = self.lock();
        st.events.force = true;
        st.events.read_at = 0.0;
        st.guide.relink_all = true;
        st.detect.force = true;
    }

    /// The viewer asks to watch the channel of a match: a request in the room (a vote when others watch), never a direct switch.
    pub fn watch(
        &self,
        user_id: &str,
        channel_id: &str,
    ) -> anyhow::Result<crate::peers::SwitchOutcome> {
        self.trace(
            "audit",
            "sport_regarder",
            json!({"canal": channel_id, "par": user_id}),
        );
        self.room.request_switch(channel_id, user_id, "sport")
    }
}

impl Sport {
    /// The live module says the channel or the status moved: look again at once.
    pub fn on_live_event(&self, topic: &str) {
        let mut st = self.lock();
        st.presence.at = 0.0;
        if topic == "live.channel" {
            st.detect.force = true;
        }
    }

    pub fn prune_trace(&self, keep_ms: i64) {
        let _ = self.store.trace_prune(self.now_ms() - keep_ms);
    }
}
