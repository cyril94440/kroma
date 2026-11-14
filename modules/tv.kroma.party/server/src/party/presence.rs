use std::collections::BTreeMap;

use super::{Party, DEPARTURE_GRACE_MS};
use crate::live::Viewer;

impl Party {
    pub(super) fn refresh_presence(&self) {
        let Some(viewers) = self.fetch_presence() else {
            return;
        };
        let now = self.now();
        let mut st = self.lock();
        let current: BTreeMap<String, String> =
            viewers.iter().map(|v| (v.user_id.clone(), v.name.clone())).collect();
        if st.seeded {
            for (id, name) in &current {
                if st.known.contains_key(id) {
                    continue;
                }
                if st.leaving.remove(id).is_some() {
                    continue;
                }
                self.system_message(&mut st, "party.joined", &[("name", name)], None);
            }
            let gone: Vec<(String, String)> = st
                .known
                .iter()
                .filter(|(id, _)| !current.contains_key(*id))
                .map(|(id, name)| (id.clone(), name.clone()))
                .collect();
            for (id, name) in gone {
                st.leaving.entry(id).or_insert((name, now + DEPARTURE_GRACE_MS));
            }
            let due: Vec<(String, String)> = st
                .leaving
                .iter()
                .filter(|(_, (_, at))| *at <= now)
                .map(|(id, (name, _))| (id.clone(), name.clone()))
                .collect();
            for (id, name) in due {
                st.leaving.remove(&id);
                if !current.contains_key(&id) {
                    self.system_message(&mut st, "party.left", &[("name", &name)], None);
                }
            }
        }
        st.seeded = true;
        st.known = current;
        st.present = Some(viewers);
        st.present_at = now;
    }

    pub fn presence(&self) -> Vec<Viewer> {
        let now = self.now();
        {
            let st = self.lock();
            if let Some(p) = st.present.as_ref() {
                if now - st.present_at < 15_000 {
                    return p.clone();
                }
            }
        }
        self.fetch_presence().unwrap_or_else(|| self.lock().present.clone().unwrap_or_default())
    }
}
