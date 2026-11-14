use std::time::Duration;

use kroma_module_sdk::host::{HostCtx, ModuleJob};

use crate::cache::{Cache, Limits};
use crate::DATA_DIR;

pub const JOB_KEY: &str = "alldebrid.cache";
pub const MAX_GB_SETTING: &str = "alldebridCacheGb";
pub const MAX_DAYS_SETTING: &str = "alldebridCacheDays";
const DEFAULT_GB: i64 = 20;
const DEFAULT_DAYS: i64 = 14;
const GB: u64 = 1024 * 1024 * 1024;
const DAY: u64 = 24 * 60 * 60;

pub fn limits_of(host: &dyn HostCtx) -> Limits {
    let gb = host.setting_i64(MAX_GB_SETTING, DEFAULT_GB).max(0) as u64;
    let days = host.setting_i64(MAX_DAYS_SETTING, DEFAULT_DAYS).max(0) as u64;
    Limits {
        max_bytes: gb.saturating_mul(GB),
        max_age: Duration::from_secs(days.saturating_mul(DAY)),
    }
}

pub fn run<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    let cache = Cache::new(&host.data_dir().join(DATA_DIR));
    cache.purge(limits_of(host));
    Ok(())
}

pub fn job<S: HostCtx + 'static>() -> ModuleJob<S> {
    ModuleJob {
        key: JOB_KEY,
        category: "maintenance",
        schedule: Some("11 * * * *"),
        run: run::<S>,
    }
}

#[cfg(test)]
mod tests {
    use kroma_module_sdk::host::testing::StubHost;
    use serde_json::json;

    use super::*;

    #[test]
    fn the_defaults_apply_until_the_operator_chooses() {
        let limits = limits_of(&StubHost::new());

        assert_eq!(limits.max_bytes, 20 * GB);
        assert_eq!(limits.max_age, Duration::from_secs(14 * DAY));
    }

    #[test]
    fn the_operators_limits_are_read_and_zero_means_unlimited() {
        let host = StubHost::new()
            .with_setting(MAX_GB_SETTING, json!(5))
            .with_setting(MAX_DAYS_SETTING, json!(0));

        let limits = limits_of(&host);

        assert_eq!(limits.max_bytes, 5 * GB);
        assert!(limits.max_age.is_zero());
    }

    #[test]
    fn the_job_is_a_maintenance_job_with_a_schedule() {
        let job = job::<StubHost>();

        assert_eq!(job.key, "alldebrid.cache");
        assert_eq!(job.category, "maintenance");
        assert!(job.schedule.is_some());
    }

    #[test]
    fn running_the_job_purges_the_modules_cache() {
        let host = StubHost::new();
        let stale = host.data_dir().join("alldebrid/cache/7/a.mkv");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "x").unwrap();
        let file = std::fs::File::options().write(true).open(&stale).unwrap();
        file.set_modified(std::time::SystemTime::now() - Duration::from_secs(40 * DAY)).unwrap();

        run(&host).unwrap();

        assert!(!stale.exists());
    }
}
