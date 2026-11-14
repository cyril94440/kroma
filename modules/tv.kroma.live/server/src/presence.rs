use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const WINDOW_MS: i64 = 60_000;
pub const HEARTBEAT_SECONDS: u64 = 15;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Viewer {
    pub user_id: String,
    pub name: String,
    pub since: i64,
}

#[derive(Debug, Clone)]
struct Seen {
    name: String,
    since: i64,
    last: i64,
}

#[derive(Debug, Default)]
pub struct Presence {
    seen: BTreeMap<String, Seen>,
    empty_since: Option<i64>,
}

impl Presence {
    pub fn beat(&mut self, user_id: &str, name: &str, now: i64) -> bool {
        let known = self.seen.contains_key(user_id);
        let entry = self.seen.entry(user_id.to_string()).or_insert_with(|| Seen { name: name.to_string(), since: now, last: now });
        entry.last = now;
        entry.name = name.to_string();
        self.empty_since = None;
        !known
    }

    pub fn leave(&mut self, user_id: &str, now: i64) -> bool {
        let removed = self.seen.remove(user_id).is_some();
        if removed && self.seen.is_empty() {
            self.empty_since = Some(now);
        }
        removed
    }

    pub fn expire(&mut self, now: i64) -> bool {
        let before = self.seen.len();
        self.seen.retain(|_, seen| now - seen.last < WINDOW_MS);
        if before > 0 && self.seen.is_empty() {
            self.empty_since = Some(now);
        }
        self.seen.len() != before
    }

    pub fn start_idle(&mut self, now: i64) {
        if self.seen.is_empty() && self.empty_since.is_none() {
            self.empty_since = Some(now);
        }
    }

    pub fn count(&self) -> usize {
        self.seen.len()
    }

    pub fn viewers(&self) -> Vec<Viewer> {
        let mut list: Vec<Viewer> = self.seen.iter().map(|(id, s)| Viewer { user_id: id.clone(), name: s.name.clone(), since: s.since }).collect();
        list.sort_by(|a, b| a.since.cmp(&b.since).then(a.user_id.cmp(&b.user_id)));
        list
    }

    pub fn idle_for_ms(&self, now: i64) -> Option<i64> {
        if self.seen.is_empty() { self.empty_since.map(|since| now - since) } else { None }
    }

    pub fn standby_due(&self, now: i64, delay_seconds: u64) -> bool {
        self.idle_for_ms(now).is_some_and(|idle| idle >= delay_seconds as i64 * 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heartbeat_keeps_a_viewer_present_for_the_window_and_no_longer() {
        let mut presence = Presence::default();
        assert!(presence.beat("u1", "Ann", 0));
        assert!(!presence.beat("u1", "Ann", 30_000));

        assert!(!presence.expire(89_999));
        assert_eq!(presence.count(), 1);
        assert!(presence.expire(90_000));
        assert_eq!(presence.count(), 0);
    }

    #[test]
    fn the_standby_clock_starts_when_the_last_viewer_goes_and_a_return_cancels_it() {
        let mut presence = Presence::default();
        presence.beat("u1", "Ann", 0);
        presence.leave("u1", 10_000);

        assert!(!presence.standby_due(99_999, 90));
        assert!(presence.standby_due(100_000, 90));
        presence.beat("u2", "Bob", 100_500);
        assert!(!presence.standby_due(500_000, 90));
        assert_eq!(presence.idle_for_ms(500_000), None);
    }

    #[test]
    fn expiry_of_the_last_viewer_also_starts_the_clock_at_that_moment() {
        let mut presence = Presence::default();
        presence.beat("u1", "Ann", 0);
        presence.expire(70_000);

        assert!(!presence.standby_due(70_000 + 89_000, 90));
        assert!(presence.standby_due(70_000 + 90_000, 90));
    }

    #[test]
    fn a_leaving_viewer_does_not_start_the_clock_while_another_is_present() {
        let mut presence = Presence::default();
        presence.beat("u1", "Ann", 0);
        presence.beat("u2", "Bob", 0);

        presence.leave("u1", 1_000);

        assert_eq!(presence.idle_for_ms(1_000_000), None);
        assert_eq!(presence.viewers().iter().map(|v| v.user_id.as_str()).collect::<Vec<_>>(), ["u2"]);
    }

    #[test]
    fn an_empty_room_at_start_begins_idle_once() {
        let mut presence = Presence::default();
        presence.start_idle(5_000);
        presence.start_idle(9_000);

        assert_eq!(presence.idle_for_ms(15_000), Some(10_000));
    }
}
