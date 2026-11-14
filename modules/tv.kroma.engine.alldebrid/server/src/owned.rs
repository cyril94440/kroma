//! Which magnets this module put in the account itself. Only those are ever
//! deleted from it: a magnet the user added on AllDebrid is theirs.

use std::path::{Path, PathBuf};

use crate::ensure_magnet_id;

#[derive(Debug, Clone)]
pub struct Owned {
    dir: PathBuf,
}

impl Owned {
    pub fn new(module_dir: &Path) -> Self {
        Self {
            dir: module_dir.join("owned"),
        }
    }

    pub fn claim(&self, magnet: &str) {
        if ensure_magnet_id(magnet).is_err() {
            return;
        }
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let _ = std::fs::write(self.dir.join(magnet), b"");
    }

    pub fn has(&self, magnet: &str) -> bool {
        ensure_magnet_id(magnet).is_ok() && self.dir.join(magnet).is_file()
    }

    pub fn release(&self, magnet: &str) {
        if ensure_magnet_id(magnet).is_ok() {
            let _ = std::fs::remove_file(self.dir.join(magnet));
        }
    }
}

/// An id is new when the snapshot taken just before the upload lacked it and it
/// is above every id the snapshot held. AllDebrid ids only grow, so a magnet
/// the listing did not show (expired, filtered) is still older than the newest
/// one it did.
pub fn is_new(before: Option<&std::collections::HashSet<String>>, id: &str) -> bool {
    let Some(before) = before else {
        return false;
    };
    let Ok(id) = id.parse::<u64>() else {
        return false;
    };
    let newest = before.iter().filter_map(|seen| seen.parse::<u64>().ok()).max();
    !before.contains(&id.to_string()) && newest.is_none_or(|newest| id > newest)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn seen(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn a_claimed_magnet_is_owned_until_released() {
        let dir = kroma_module_sdk::testing::temp_dir("ad-owned");
        let owned = Owned::new(dir.path());

        assert!(!owned.has("7"));
        owned.claim("7");
        assert!(owned.has("7"));
        owned.release("7");
        assert!(!owned.has("7"));
    }

    #[test]
    fn a_hostile_id_is_never_owned() {
        let dir = kroma_module_sdk::testing::temp_dir("ad-owned-bad");
        let owned = Owned::new(dir.path());

        owned.claim("../x");

        assert!(!owned.has("../x"));
        assert!(!dir.path().join("x").exists());
    }

    #[test]
    fn only_an_id_above_everything_seen_before_is_new() {
        let before = seen(&["5", "9"]);

        assert!(is_new(Some(&before), "10"));
        assert!(!is_new(Some(&before), "9"));
        assert!(!is_new(Some(&before), "7"));
    }

    #[test]
    fn into_an_empty_account_every_id_is_new_and_with_no_snapshot_none_is() {
        assert!(is_new(Some(&seen(&[])), "1"));
        assert!(!is_new(None, "1"));
        assert!(!is_new(Some(&seen(&[])), "x"));
    }
}
