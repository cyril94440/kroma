use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub(super) const ROOT_TTL: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(super) struct Roots {
    found: Mutex<HashMap<(PathBuf, String), (Instant, PathBuf)>>,
}

impl Roots {
    pub(super) fn get(
        &self,
        data_dir: &std::path::Path,
        module: &str,
        now: Instant,
        compute: impl FnOnce() -> Option<PathBuf>,
    ) -> Option<PathBuf> {
        let key = (data_dir.to_path_buf(), module.to_string());
        let mut found = self.found.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, root)) = found.get(&key) {
            if now.saturating_duration_since(*at) < ROOT_TTL {
                return Some(root.clone());
            }
        }
        found.remove(&key);
        let root = compute()?;
        found.insert(key, (now, root.clone()));
        Some(root)
    }

    pub(super) fn forget(&self, data_dir: &std::path::Path, module: &str) {
        self.found
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(data_dir.to_path_buf(), module.to_string()));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn a_root_is_looked_up_once_per_window_and_a_miss_is_never_remembered() {
        let roots = Roots::default();
        let data = std::path::Path::new("/data");
        let calls = Cell::new(0);
        let t0 = Instant::now();
        let lookup = |at: Instant, answer: Option<&str>| {
            roots.get(data, "tv.kroma.live", at, || {
                calls.set(calls.get() + 1);
                answer.map(PathBuf::from)
            })
        };

        assert_eq!(lookup(t0, None), None);
        assert_eq!(lookup(t0, Some("/data/live/public")), Some(PathBuf::from("/data/live/public")));
        assert_eq!(lookup(t0 + Duration::from_secs(5), Some("/data/other")), Some(PathBuf::from("/data/live/public")));
        assert_eq!(calls.get(), 2);
        assert_eq!(lookup(t0 + ROOT_TTL, Some("/data/other")), Some(PathBuf::from("/data/other")));
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn forgetting_a_module_makes_the_next_request_look_it_up_again() {
        let roots = Roots::default();
        let data = std::path::Path::new("/data");
        let t0 = Instant::now();
        roots.get(data, "m", t0, || Some(PathBuf::from("/a")));

        roots.forget(data, "m");

        assert_eq!(roots.get(data, "m", t0, || Some(PathBuf::from("/b"))), Some(PathBuf::from("/b")));
    }

    #[test]
    fn two_servers_sharing_a_process_never_share_a_root() {
        let roots = Roots::default();
        let t0 = Instant::now();
        roots.get(std::path::Path::new("/one"), "m", t0, || Some(PathBuf::from("/one/public")));

        let other = roots.get(std::path::Path::new("/two"), "m", t0, || Some(PathBuf::from("/two/public")));

        assert_eq!(other, Some(PathBuf::from("/two/public")));
    }
}
