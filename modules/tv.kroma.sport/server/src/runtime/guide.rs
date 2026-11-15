use serde_json::json;

use super::state::MEMORY_KEY;
use super::Sport;
use crate::peers::Programme;
use crate::sport::{
    channel_in_text, choose_programme, guide_queries, is_foreign_channel, ChannelLink,
    GuideProgramme, SportEvent, State,
};

const GUIDE_TTL_S: f64 = 1800.0;
const GUIDE_EMPTY_TTL_S: f64 = 600.0;
const PER_TICK: usize = 2;
const TARGETS_MAX: usize = 60;
const HORIZON_S: f64 = 36.0 * 3600.0;
const PREFERENCE_TTL_S: f64 = 3600.0;

fn as_guide(p: &Programme) -> GuideProgramme {
    GuideProgramme {
        channel_id: p.channel_id.clone(),
        channel_name: p.channel_name.clone(),
        title: p.title.clone(),
        description: p.description.clone(),
        start_s: p.start as f64 / 1000.0,
        end_s: p.end as f64 / 1000.0,
    }
}

impl Sport {
    pub(crate) fn step_guide(&self, now: f64) -> bool {
        let targets: Vec<SportEvent> = {
            let mut st = self.lock();
            if now - st.guide.preference_at > PREFERENCE_TTL_S {
                st.guide.preference.clear();
                st.guide.preference_at = now;
            }
            let relink = std::mem::take(&mut st.guide.relink_all);
            if relink {
                st.guide.empty.clear();
                st.guide.links.retain(|_, l| l.source != "guide");
            }
            let mut targets: Vec<SportEvent> = st
                .events
                .list
                .iter()
                .filter(|e| e.state != State::Post && e.start - now < HORIZON_S)
                .cloned()
                .collect();
            targets.sort_by_key(|e| (e.state != State::In, -e.popularity));
            targets.truncate(TARGETS_MAX);
            targets
                .into_iter()
                .filter(|e| {
                    st.guide
                        .links
                        .get(&e.id)
                        .is_none_or(|l| now - l.at >= GUIDE_TTL_S)
                        && now - st.guide.empty.get(&e.id).copied().unwrap_or(0.0)
                            >= GUIDE_EMPTY_TTL_S
                })
                .take(PER_TICK)
                .collect()
        };
        if targets.is_empty() {
            return false;
        }
        let home = self.settings().home_prefixes;
        for event in &targets {
            self.link_event(event, now, &home);
        }
        true
    }

    fn link_event(&self, event: &SportEvent, now: f64, home: &[String]) {
        let (from, to) = (
            ((event.start - 3.0 * 3600.0) * 1000.0) as i64,
            ((event.start + 4.0 * 3600.0) * 1000.0) as i64,
        );
        let mut choice = None;
        for query in guide_queries(event) {
            let programmes: Vec<GuideProgramme> = self
                .live
                .epg(&query, from, to, 200)
                .map(|list| list.iter().map(as_guide).collect())
                .unwrap_or_default();
            choice = choose_programme(&programmes, event, now, |name| self.preference(name));
            if choice.is_some() {
                break;
            }
        }
        let mut st = self.lock();
        match choice {
            Some(found) => {
                let foreign = is_foreign_channel(&found.channel_name, home);
                st.guide.empty.remove(&event.id);
                st.guide.links.insert(
                    event.id.clone(),
                    ChannelLink {
                        name: found.channel_name,
                        channel_id: Some(found.channel_id),
                        source: "guide".into(),
                        at: now,
                        foreign,
                    },
                );
            }
            None => {
                if st.guide.empty.len() > 500 {
                    st.guide.empty.clear();
                }
                st.guide.empty.insert(event.id.clone(), now);
                if st
                    .guide
                    .links
                    .get(&event.id)
                    .is_some_and(|l| l.source == "guide")
                {
                    st.guide.links.remove(&event.id);
                }
            }
        }
        self.store.kv_put(super::state::LINKS_KEY, &st.guide.links);
    }

    fn preference(&self, channel_name: &str) -> f64 {
        if let Some(known) = self.lock().guide.preference.get(channel_name) {
            return *known;
        }
        let score = self
            .live
            .resolve(channel_name, 3)
            .ok()
            .and_then(|found| {
                found
                    .into_iter()
                    .find(|c| c.name == channel_name)
                    .map(|c| c.score)
            })
            .unwrap_or(0.0);
        self.lock()
            .guide
            .preference
            .insert(channel_name.to_string(), score);
        score
    }

    /// A web answer naming the channel of a match (read by the model, decided by code): kept as a hint for three hours.
    pub fn note_channel_hint(
        &self,
        home: &str,
        away: &str,
        answer: &str,
    ) -> Result<String, &'static str> {
        let now = self.now();
        let Some(channel) = channel_in_text(answer) else {
            return Err("aucuneChaine");
        };
        let event = {
            let st = self.lock();
            st.events
                .list
                .iter()
                .find(|e| {
                    e.team_names().is_some_and(|(a, b)| {
                        crate::sport::title_covers(&format!("{a} / {b}"), home, away)
                            || crate::sport::title_covers(&format!("{a} / {b}"), away, home)
                    })
                })
                .cloned()
        };
        let Some(event) = event else {
            return Err("matchIntrouvable");
        };
        let resolved = self
            .live
            .resolve(&channel, 1)
            .ok()
            .and_then(|found| found.into_iter().next());
        let Some(resolved) = resolved else {
            return Err("chaineIntrouvable");
        };
        let mut st = self.lock();
        st.memory.web.insert(
            event.id.clone(),
            crate::sport::WebHint {
                query: Some(channel),
                at: now,
            },
        );
        if st.guide.links.get(&event.id).is_none_or(|l| l.foreign) {
            let foreign = is_foreign_channel(&resolved.name, &self.settings().home_prefixes);
            st.guide.links.insert(
                event.id.clone(),
                ChannelLink {
                    name: resolved.name.clone(),
                    channel_id: Some(resolved.id),
                    source: "web".into(),
                    at: now,
                    foreign,
                },
            );
        }
        self.store.kv_put(MEMORY_KEY, &st.memory);
        self.store.kv_put(super::state::LINKS_KEY, &st.guide.links);
        drop(st);
        self.trace(
            "espn",
            "web_hint",
            json!({"event": event.id, "chaine": resolved.name}),
        );
        Ok(event.id)
    }
}
