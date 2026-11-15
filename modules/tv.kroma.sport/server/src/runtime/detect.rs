use serde_json::json;

use super::board::BoardStep;
use super::state::Matched;
use super::Sport;
use crate::detect::{contradicts, decide, DetectProgramme, FlatMatch, Verdict, POST_WINDOW_S};
use crate::peers::Programme;
use crate::sport::{Certainty, Reading};
use crate::text::norm_lossy;

const KEEP_POST_S: f64 = POST_WINDOW_S;
const IMAGE_FRESH_S: f64 = 600.0;

fn as_detect(p: &Programme) -> DetectProgramme {
    DetectProgramme {
        title: p.title.clone(),
        hay: norm_lossy(&format!("{} {}", p.title, p.description)),
        start: Some(p.start as f64 / 1000.0),
        stop: Some(p.end as f64 / 1000.0),
    }
}

impl Sport {
    pub(crate) fn step_detect(&self, now: f64) -> bool {
        let settings = self.settings();
        if !self.watched() {
            return false;
        }
        {
            let st = self.lock();
            let Some(_) = st.presence.channel.as_ref() else {
                return false;
            };
            if !st.detect.force && now - st.detect.last_run < settings.detection_s {
                return false;
            }
        }
        match self.step_board(now) {
            BoardStep::Fetched => true,
            BoardStep::Failed => {
                self.lock().detect.last_run = now;
                true
            }
            BoardStep::Ready(matches) => {
                self.decide_direct(&matches, now);
                false
            }
        }
    }

    fn decide_direct(&self, matches: &[FlatMatch], now: f64) {
        let channel = {
            let mut st = self.lock();
            st.detect.last_run = now;
            st.detect.force = false;
            let Some(channel) = st.presence.channel.clone() else {
                return;
            };
            for m in matches.iter().filter(|m| m.state == "post") {
                st.detect.post_seen.entry(m.id.clone()).or_insert(now);
            }
            channel
        };
        let image = self.recent_reading(&channel.id, now);
        if self.keep_current(matches, &channel.id, image.as_ref(), now) {
            return;
        }
        let programmes = self.channel_programmes(&channel.id, now);
        let post_seen = self.lock().detect.post_seen.clone();
        let decision = decide(matches, &programmes, now, image.as_ref(), &post_seen);
        let remembered = self.remembered_match(matches, &channel.id);
        let chosen = match (decision.verdict, decision.matched.clone(), remembered) {
            (Verdict::Ok, Some(found), _) => Some(found),
            (_, _, Some(found)) => Some(found),
            _ => None,
        };
        self.trace(
            "match",
            "match_detecte",
            json!({"chaine": channel.name, "statut": decision.verdict, "score": decision.score, "candidats": decision.candidates, "raisons": decision.reasons, "retenu": chosen.as_ref().map(FlatMatch::label)}),
        );
        let mut st = self.lock();
        st.detect.last_verdict = format!("{:?}", decision.verdict).to_lowercase();
        let changed =
            st.detect.matched.as_ref().map(|m| &m.found.id) != chosen.as_ref().map(|c| &c.id);
        st.detect.matched = chosen.map(|found| Matched {
            found,
            channel_id: channel.id.clone(),
        });
        if changed {
            st.follow.last_fetch = 0.0;
        }
    }

    fn recent_reading(&self, channel_id: &str, now: f64) -> Option<Reading> {
        let st = self.lock();
        st.memory
            .image
            .get(channel_id)
            .filter(|s| now - s.at <= IMAGE_FRESH_S)
            .and_then(|s| s.reading.clone())
    }

    fn keep_current(
        &self,
        matches: &[FlatMatch],
        channel_id: &str,
        image: Option<&Reading>,
        now: f64,
    ) -> bool {
        let mut st = self.lock();
        let Some(current) = st.detect.matched.clone() else {
            return false;
        };
        if current.channel_id != channel_id {
            return false;
        }
        let fresh = matches
            .iter()
            .find(|m| m.id == current.found.id)
            .cloned()
            .unwrap_or(current.found.clone());
        let ended_for = st
            .detect
            .post_seen
            .get(&fresh.id)
            .filter(|_| fresh.state == "post")
            .map(|at| now - at);
        let wrong = contradicts(&fresh, image, matches);
        if wrong || ended_for.is_some_and(|d| d > KEEP_POST_S) {
            st.detect.matched = None;
            return false;
        }
        st.detect.matched = Some(Matched {
            found: fresh,
            ..current
        });
        true
    }

    fn channel_programmes(&self, channel_id: &str, now: f64) -> Vec<DetectProgramme> {
        let status = self.live.status().ok();
        let title = status
            .as_ref()
            .and_then(|s| s.now.as_ref())
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let from = ((now - 3.0 * 3600.0) * 1000.0) as i64;
        let to = ((now + 3.0 * 3600.0) * 1000.0) as i64;
        let mut out: Vec<DetectProgramme> = if title.is_empty() {
            Vec::new()
        } else {
            self.live
                .epg(&title, from, to, 80)
                .unwrap_or_default()
                .iter()
                .filter(|p| p.channel_id == channel_id)
                .map(as_detect)
                .collect()
        };
        if let Some(status) = status {
            for slot in [status.now, status.next].into_iter().flatten() {
                if !out
                    .iter()
                    .any(|p| p.start == Some(slot.start as f64 / 1000.0))
                {
                    out.push(DetectProgramme {
                        title: slot.title.clone(),
                        hay: norm_lossy(&slot.title),
                        start: Some(slot.start as f64 / 1000.0),
                        stop: Some(slot.end as f64 / 1000.0),
                    });
                }
            }
        }
        out
    }

    fn remembered_match(&self, matches: &[FlatMatch], channel_id: &str) -> Option<FlatMatch> {
        let st = self.lock();
        let seen = st
            .memory
            .image
            .get(channel_id)
            .filter(|s| s.certainty == Some(Certainty::Seen))?;
        let event = st
            .events
            .list
            .iter()
            .find(|e| Some(&e.id) == seen.event.as_ref())?;
        matches
            .iter()
            .find(|m| m.id == format!("espn-{}", event.espn) && m.state != "post")
            .cloned()
    }
}
