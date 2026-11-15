use std::collections::BTreeMap;

use crate::detect::FlatMatch;

const BUFFER_MAX: usize = 200;
const KEEP_EXTRA_S: f64 = 600.0;

#[derive(Debug, Default)]
pub struct Lag {
    buffer: Vec<(f64, String, Vec<FlatMatch>)>,
    served: BTreeMap<String, (f64, Vec<FlatMatch>)>,
}

impl Lag {
    pub fn read(
        &mut self,
        key: &str,
        fresh: Option<Vec<FlatMatch>>,
        now: f64,
        delay_s: f64,
        default_s: f64,
    ) -> Option<Vec<FlatMatch>> {
        if let Some(matches) = fresh {
            self.buffer.push((now, key.to_string(), matches));
        }
        let excess = self.buffer.len().saturating_sub(BUFFER_MAX);
        self.buffer.drain(..excess);
        let horizon = now - delay_s.max(default_s) - KEEP_EXTRA_S;
        self.buffer.retain(|(t, _, _)| *t >= horizon);
        let ready = self
            .buffer
            .iter()
            .rfind(|(t, k, _)| k == key && *t <= now - delay_s)
            .map(|(t, _, m)| (*t, m.clone()));
        let previous = self.served.get(key).filter(|(t, _)| *t >= horizon).cloned();
        match (ready, previous) {
            (Some(ready), previous) if previous.as_ref().is_none_or(|p| ready.0 >= p.0) => {
                self.served.insert(key.to_string(), ready.clone());
                Some(ready.1)
            }
            (_, previous) => previous.map(|p| p.1),
        }
    }
}
