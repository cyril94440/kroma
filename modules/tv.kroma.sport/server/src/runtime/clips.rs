use serde_json::{json, Value};

use super::Sport;
use crate::clips::{review, to_cut, Window};
use crate::store::{ClipRow, NewClip};

const BUFFER_POLL_S: f64 = 30.0;
const UNSUPPORTED_RETRY_S: f64 = 300.0;
const CUT_GAP_S: f64 = 2.0;
const KEEP_MAX: u32 = 300;
const ATTEMPTS_KEEP_MS: i64 = 7 * 86_400_000;

impl Sport {
    pub(crate) fn step_clips(&self, now: f64) -> bool {
        let settings = self.settings();
        let goals = {
            let st = self.lock();
            if !settings.clips || now < st.clips.unsupported_until {
                return false;
            }
            let Some(payload) = st.follow.payload.clone() else {
                return false;
            };
            if !payload.facts.iter().any(|f| f.kind == "but") || now - st.clips.last_cut < CUT_GAP_S
            {
                return false;
            }
            (payload, st.follow.summary.clone())
        };
        self.refresh_buffer(now);
        let (payload, summary) = goals;
        let reviews = {
            let mut st = self.lock();
            let (delay, _) = st.pronos.delay(now);
            review(
                &payload,
                &summary,
                now,
                delay,
                &st.clips.done,
                &st.clips.attempts,
                st.clips.buffer_from_s,
            )
        };
        self.trace_goal_states(&reviews);
        let Some(next) = to_cut(reviews).into_iter().next() else {
            return false;
        };
        let Some(window) = next.window else {
            return false;
        };
        self.cut(&next.id, window, &next.label, now)
    }

    fn refresh_buffer(&self, now: f64) {
        if now - self.lock().clips.buffer_at < BUFFER_POLL_S {
            return;
        }
        let info = self.live.clip_buffer_info().ok().flatten();
        let mut st = self.lock();
        st.clips.buffer_at = now;
        st.clips.buffer_from_s = info.and_then(|i| i.from_ms).map(|ms| ms as f64 / 1000.0);
    }

    fn trace_goal_states(&self, reviews: &[crate::clips::GoalReview]) {
        let mut changed = Vec::new();
        {
            let mut st = self.lock();
            for r in reviews {
                let state = format!("{:?}", r.state);
                if st.clips.states.get(&r.id) != Some(&state) {
                    st.clips.states.insert(r.id.clone(), state.clone());
                    changed.push(json!({"id": r.id, "etat": state, "minute": r.label.minute, "periode": r.period, "periode_calee": r.period_calibrated, "t_image": r.t_image, "t_vu": r.t_vu, "heure_espn": r.espn_time_s, "fenetre": r.window.map(|w| json!({"de": w.from_s, "a": w.to_s, "pivot": w.pivot_s, "source": if w.from_image { "image" } else { "espn" }}))}));
                }
            }
        }
        for line in changed {
            self.trace("clips", "but", line);
        }
    }

    fn cut(&self, id: &str, window: Window, label: &crate::clips::Label, now: f64) -> bool {
        let (from_ms, to_ms) = (
            (window.from_s * 1000.0) as i64,
            (window.to_s * 1000.0) as i64,
        );
        let outcome = self.live.clip_cut(from_ms, to_ms, id);
        let mut st = self.lock();
        st.clips.last_cut = now;
        match outcome {
            Ok(Some(cut)) => {
                let row = NewClip {
                    id: id.to_string(),
                    match_id: label.match_id.clone(),
                    ev_id: label.ev_id.clone(),
                    dom: label.dom.clone(),
                    ext: label.ext.clone(),
                    competition: label.competition.clone(),
                    minute: label.minute.clone(),
                    equipe: label.equipe.clone(),
                    buteur: label.buteur.clone(),
                    score: label.score.clone(),
                    debut_ms: from_ms,
                    fin_ms: to_ms,
                    duree_ms: cut.duration_ms,
                    path: cut.path,
                    ts: (now * 1000.0) as i64,
                };
                if let Err(e) = self.store.clip_insert(&row) {
                    tracing::warn!(error = %e, "clip not stored");
                }
                st.clips.done.insert(id.to_string(), now);
                drop(st);
                self.trace("clips", "fait", json!({"id": id, "source": if window.from_image { "image" } else { "espn" }, "duree_ms": cut.duration_ms}));
                self.publish_clips();
            }
            Ok(None) => {
                st.clips.unsupported_until = now + UNSUPPORTED_RETRY_S;
                drop(st);
                self.trace("clips", "indisponible", json!({"id": id, "raison": "le module live ne sait pas encore decouper (clipCut)"}));
            }
            Err(e) => {
                *st.clips.attempts.entry(id.to_string()).or_default() += 1;
                drop(st);
                let _ = self.store.attempt_add(id, self.now_ms());
                self.trace("clips", "echec", json!({"id": id, "erreur": e.to_string()}));
            }
        }
        true
    }

    pub fn clips(&self, match_id: Option<&str>, limit: u32) -> Vec<Value> {
        self.store
            .clips(match_id, limit.min(KEEP_MAX))
            .unwrap_or_default()
            .iter()
            .map(clip_json)
            .collect()
    }

    pub(crate) fn publish_clips(&self) {
        let list = self.clips(None, KEEP_MAX);
        let fingerprint = list
            .iter()
            .map(|c| c["id"].as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut st = self.lock();
        self.emit_if_changed(&mut st, "sport.clips", json!({"clips": list}), fingerprint);
    }

    /// Drops the metadata of clips past retention or whose file the live module no longer holds, and asks it to delete the file when it can.
    pub fn reconcile_clips(&self) {
        let now_ms = self.now_ms();
        let days = self.settings().clip_retention_days;
        let mut gone: Vec<ClipRow> = self
            .store
            .clips_older_than(now_ms - days * 86_400_000)
            .unwrap_or_default();
        gone.extend(self.store.clips_over(KEEP_MAX).unwrap_or_default());
        if let Ok(Some(present)) = self.live.clip_list() {
            gone.extend(
                self.store
                    .clips(None, 1000)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|c| !present.contains(&c.path)),
            );
        }
        for clip in gone {
            let _ = self.live.clip_delete(&clip.path);
            let _ = self.store.clip_delete(&clip.id);
            self.lock().clips.done.remove(&clip.id);
        }
        let _ = self.store.attempts_prune(now_ms - ATTEMPTS_KEEP_MS);
        self.publish_clips();
    }

    pub fn delete_clip(&self, id: &str) -> bool {
        let Ok(Some(clip)) = self.store.clip(id) else {
            return false;
        };
        let _ = self.live.clip_delete(&clip.path);
        let _ = self.store.clip_delete(id);
        self.publish_clips();
        true
    }
}

pub(crate) fn clip_json(c: &ClipRow) -> Value {
    json!({
        "id": c.id, "match_id": c.match_id, "ev_id": c.ev_id, "dom": c.dom, "ext": c.ext, "competition": c.competition,
        "minute": c.minute, "equipe": c.equipe, "buteur": c.buteur, "score": c.score,
        "debut_ms": c.debut_ms, "fin_ms": c.fin_ms, "duree_s": (c.duree_ms as f64 / 1000.0).round() as i64,
        "octets": 0, "vignette": false, "ts": c.ts,
        "file": {"module": "tv.kroma.live", "path": c.path},
    })
}
