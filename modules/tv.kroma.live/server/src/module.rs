use kroma_module_sdk::host::{service, HostCtx, ModuleJob, ServerModule};

use crate::db;
use crate::runtime::Live;

pub const CATALOGUE_JOB: &str = "live.catalogue";
pub const GUIDE_JOB: &str = "live.guide";

pub struct LiveModule;

fn refresh_catalogue<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    let Some(live) = service::<Live>(host) else { return Ok(()) };
    let failures: Vec<String> = live.refresh_catalogues().into_iter().filter_map(|(_, result)| result.err()).collect();
    if failures.is_empty() { Ok(()) } else { anyhow::bail!("{}", failures.join("; ")) }
}

fn refresh_guide<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    let Some(live) = service::<Live>(host) else { return Ok(()) };
    live.refresh_epg().map(|_| ())
}

#[kroma_module_sdk::host::async_trait]
impl<S: HostCtx + Clone + Send + Sync + 'static> ServerModule<S> for LiveModule {
    fn id(&self) -> &'static str {
        crate::MODULE_ID
    }

    fn migrations(&self) -> &'static str {
        db::MIGRATIONS
    }

    fn admin_routes(&self, _host: &S) -> Option<axum::Router<S>> {
        Some(crate::routes::routes::<S>())
    }

    fn jobs(&self) -> Vec<ModuleJob<S>> {
        vec![
            ModuleJob { key: CATALOGUE_JOB, category: "maintenance", schedule: Some("19 * * * *"), run: refresh_catalogue::<S> },
            ModuleJob { key: GUIDE_JOB, category: "maintenance", schedule: Some("7 */3 * * *"), run: refresh_guide::<S> },
        ]
    }

    async fn on_enable(&self, host: S) {
        if let Some(live) = service::<Live>(&host) {
            live.start();
        }
    }

    async fn on_disable(&self, host: S) {
        if let Some(live) = service::<Live>(&host) {
            live.shutdown();
        }
    }
}

pub fn server_module<S: HostCtx + Clone + Send + Sync + 'static>() -> Box<dyn ServerModule<S>> {
    Box::new(LiveModule)
}
