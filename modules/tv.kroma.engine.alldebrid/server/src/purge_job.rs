use kroma_module_sdk::host::{HostCtx, ModuleJob};

use crate::cache::Cache;
use crate::settings::CacheSettings;
use crate::DATA_DIR;

pub const JOB_KEY: &str = "alldebrid.cache";

pub fn run<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    let cache = Cache::new(&host.data_dir().join(DATA_DIR));
    cache.purge(CacheSettings::read(host).limits());
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
    use std::time::Duration;

    use kroma_module_sdk::host::testing::StubHost;
    use serde_json::json;

    use super::*;

    fn stale_file(host: &StubHost, days: u64) -> std::path::PathBuf {
        let path = host.data_dir().join("alldebrid/cache/7/a.mkv");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x").unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(std::time::SystemTime::now() - Duration::from_secs(days * 86_400)).unwrap();
        path
    }

    #[test]
    fn the_job_is_a_maintenance_job_with_a_schedule() {
        let job = job::<StubHost>();

        assert_eq!(job.key, "alldebrid.cache");
        assert_eq!(job.category, "maintenance");
        assert!(job.schedule.is_some());
    }

    #[test]
    fn running_the_job_purges_what_has_sat_unused_too_long() {
        let host = StubHost::new();
        let stale = stale_file(&host, 40);

        run(&host).unwrap();

        assert!(!stale.exists());
    }

    #[test]
    fn running_the_job_keeps_a_recent_file_within_budget() {
        let host = StubHost::new();
        let fresh = stale_file(&host, 1);

        run(&host).unwrap();

        assert!(fresh.exists());
    }

    #[test]
    fn with_the_cache_switched_off_the_job_empties_it() {
        let host = StubHost::new().with_setting("cacheMaxGb", json!(0));
        let idle = stale_file(&host, 1);

        run(&host).unwrap();

        assert!(!idle.exists());
    }
}
