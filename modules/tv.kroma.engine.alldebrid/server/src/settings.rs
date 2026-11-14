use std::time::Duration;

use kroma_module_sdk::host::HostCtx;

use crate::cache::Limits;

pub const MAX_GB_KEY: &str = "cacheMaxGb";
pub const MAX_DAYS_KEY: &str = "cacheMaxDays";
const DEFAULT_GB: i64 = 50;
const DEFAULT_DAYS: i64 = 14;
const GB: u64 = 1024 * 1024 * 1024;
const DAY: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheSettings {
    pub max_gb: u64,
    pub max_days: u64,
}

impl Default for CacheSettings {
    fn default() -> Self {
        Self { max_gb: DEFAULT_GB as u64, max_days: DEFAULT_DAYS as u64 }
    }
}

impl CacheSettings {
    pub fn read(host: &dyn HostCtx) -> Self {
        Self {
            max_gb: host.setting_i64(MAX_GB_KEY, DEFAULT_GB).max(0) as u64,
            max_days: host.setting_i64(MAX_DAYS_KEY, DEFAULT_DAYS).max(0) as u64,
        }
    }

    pub fn enabled(&self) -> bool {
        self.max_gb > 0
    }

    /// Switched off, the limits evict everything that is not in use.
    pub fn limits(&self) -> Limits {
        if !self.enabled() {
            return Limits { max_bytes: Some(0), max_age: Duration::ZERO };
        }
        Limits {
            max_bytes: Some(self.max_gb.saturating_mul(GB)),
            max_age: Duration::from_secs(self.max_days.saturating_mul(DAY)),
        }
    }
}

#[cfg(test)]
mod tests {
    use kroma_module_sdk::host::testing::StubHost;
    use serde_json::json;

    use super::*;

    #[test]
    fn the_defaults_apply_until_the_operator_chooses() {
        let settings = CacheSettings::read(&StubHost::new());

        assert_eq!(settings, CacheSettings { max_gb: 50, max_days: 14 });
        assert_eq!(settings.limits().max_bytes, Some(50 * GB));
        assert_eq!(settings.limits().max_age, Duration::from_secs(14 * DAY));
    }

    #[test]
    fn the_operators_values_are_read_and_zero_days_means_no_age_limit() {
        let host = StubHost::new()
            .with_setting(MAX_GB_KEY, json!(5))
            .with_setting(MAX_DAYS_KEY, json!(0));

        let settings = CacheSettings::read(&host);

        assert_eq!(settings.limits().max_bytes, Some(5 * GB));
        assert!(settings.limits().max_age.is_zero());
    }

    #[test]
    fn zero_gigabytes_is_no_cache_and_evicts_everything_idle() {
        let settings = CacheSettings { max_gb: 0, max_days: 14 };

        assert!(!settings.enabled());
        assert_eq!(settings.limits().max_bytes, Some(0));
    }

    #[test]
    fn a_negative_value_reads_as_zero() {
        let host = StubHost::new().with_setting(MAX_GB_KEY, json!(-3));

        assert_eq!(CacheSettings::read(&host).max_gb, 0);
    }
}
