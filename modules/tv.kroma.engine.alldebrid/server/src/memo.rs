//! Short-lived answers kept in memory: a burst of lookups for one file costs
//! one call to AllDebrid, and nothing outlives the time its answer is good for.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use crate::files::Planned;

pub const URL_TTL: Duration = Duration::from_secs(3 * 60);
pub const PLAN_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_ENTRIES: usize = 512;

pub struct Memo<V> {
    ttl: Duration,
    items: Mutex<HashMap<String, (Instant, V)>>,
}

impl<V: Clone> Memo<V> {
    pub fn new(ttl: Duration) -> Self {
        Self { ttl, items: Mutex::default() }
    }

    pub fn get(&self, key: &str, now: Instant) -> Option<V> {
        let items = self.items.lock().unwrap_or_else(PoisonError::into_inner);
        let (stored, value) = items.get(key)?;
        (now.saturating_duration_since(*stored) < self.ttl).then(|| value.clone())
    }

    pub fn put(&self, key: String, value: V, now: Instant) {
        let mut items = self.items.lock().unwrap_or_else(PoisonError::into_inner);
        items.retain(|_, (stored, _)| now.saturating_duration_since(*stored) < self.ttl);
        if items.len() >= MAX_ENTRIES {
            items.clear();
        }
        items.insert(key, (now, value));
    }

    pub fn forget(&self, key: &str) {
        self.items.lock().unwrap_or_else(PoisonError::into_inner).remove(key);
    }
}

pub fn urls() -> &'static Memo<String> {
    static URLS: OnceLock<Memo<String>> = OnceLock::new();
    URLS.get_or_init(|| Memo::new(URL_TTL))
}

pub fn plans() -> &'static Memo<Vec<Planned>> {
    static PLANS: OnceLock<Memo<Vec<Planned>>> = OnceLock::new();
    PLANS.get_or_init(|| Memo::new(PLAN_TTL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_is_served_until_its_time_is_up() {
        let memo = Memo::new(Duration::from_secs(60));
        let start = Instant::now();
        memo.put("a".into(), 1, start);

        assert_eq!(memo.get("a", start + Duration::from_secs(59)), Some(1));
        assert_eq!(memo.get("a", start + Duration::from_secs(60)), None);
        assert_eq!(memo.get("b", start), None);
    }

    #[test]
    fn a_forgotten_answer_is_gone() {
        let memo = Memo::new(Duration::from_secs(60));
        let start = Instant::now();
        memo.put("a".into(), 1, start);

        memo.forget("a");

        assert_eq!(memo.get("a", start), None);
    }

    #[test]
    fn a_full_memo_makes_room_rather_than_growing() {
        let memo = Memo::new(Duration::from_secs(60));
        let start = Instant::now();
        for n in 0..=MAX_ENTRIES {
            memo.put(n.to_string(), n, start);
        }

        assert!(memo.items.lock().unwrap().len() <= MAX_ENTRIES);
        assert_eq!(memo.get(&MAX_ENTRIES.to_string(), start), Some(MAX_ENTRIES));
    }

    #[test]
    fn the_unlocked_links_expire_well_inside_the_plans() {
        assert!(URL_TTL <= Duration::from_secs(5 * 60));
        assert!(URL_TTL < PLAN_TTL);
    }
}
