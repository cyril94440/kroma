#![allow(clippy::result_large_err)]

#[cfg(target_family = "wasm")]
extern crate kroma_sqlite_guest as rusqlite;

mod cards;
mod clips;
mod clock;
mod detect;
mod espn;
mod iso;
mod live_match;
mod match_facts;
mod module;
mod peers;
mod prono;
mod routes;
mod runtime;
mod settings;
mod sport;
mod standings;
mod store;
mod strings;
mod team_match;
mod text;
mod tools;

use std::sync::Arc;

use kroma_module_sdk::host::{service, HostCtx};

pub use clock::{system_clock, Clock};
pub use module::MODULE;
pub use peers::{CONTROL, ROOM};
pub use routes::routes;
pub use runtime::Sport;
pub use tools::{routes as tool_routes, TOOLS};

pub const MODULE_ID: &str = "tv.kroma.sport";

const TOPICS: [&str; 2] = ["live.channel", "live.status"];
const TRACE_KEEP_MS: i64 = 30 * 86_400_000;

/// Builds the sport service over the host's peers and its own storage.
pub fn build(host: Arc<dyn HostCtx>, pool: store::Pool) -> Arc<Sport> {
    let base_host = host.clone();
    let espn = Arc::new(espn::HttpEspn::new(move || {
        settings::Settings::read(&*base_host).espn_base
    }));
    Sport::new(
        host.clone(),
        store::Store::new(pool),
        espn,
        Arc::new(peers::PeerLive::new(host.clone())),
        Arc::new(peers::PeerRoom::new(host)),
    )
}

pub struct SportModule;

fn reconcile_job<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    if let Some(sport) = service::<Sport>(host) {
        sport.reconcile_clips();
        sport.refresh();
    }
    Ok(())
}

fn maintenance_job<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    if let Some(sport) = service::<Sport>(host) {
        sport.reconcile_clips();
        sport.prune_trace(TRACE_KEEP_MS);
    }
    Ok(())
}

#[kroma_module_sdk::host::async_trait]
impl<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static>
    kroma_module_sdk::host::ServerModule<S> for SportModule
{
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn migrations(&self) -> &'static str {
        store::MIGRATIONS
    }

    fn admin_routes(&self, host: &S) -> Option<axum::Router<S>> {
        service::<Sport>(host).map(routes::<S>)
    }

    fn jobs(&self) -> Vec<kroma_module_sdk::host::ModuleJob<S>> {
        vec![
            kroma_module_sdk::host::ModuleJob {
                key: "sport.reconcile",
                category: "maintenance",
                schedule: Some("*/15 * * * *"),
                run: reconcile_job::<S>,
            },
            kroma_module_sdk::host::ModuleJob {
                key: "sport.maintenance",
                category: "maintenance",
                schedule: Some("20 4 * * *"),
                run: maintenance_job::<S>,
            },
        ]
    }

    fn events(&self) -> Vec<&'static str> {
        TOPICS.to_vec()
    }

    async fn on_event(&self, host: S, topic: String, _payload: serde_json::Value) {
        if let Some(sport) = service::<Sport>(&host) {
            let _ = kroma_module_sdk::host::run_blocking(move || sport.on_live_event(&topic)).await;
        }
    }

    async fn on_enable(&self, host: S) {
        if let Some(sport) = service::<Sport>(&host) {
            sport.start();
        }
    }

    async fn on_disable(&self, host: S) {
        if let Some(sport) = service::<Sport>(&host) {
            sport.stop();
        }
    }
}

pub fn server_module<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static>(
) -> Box<dyn kroma_module_sdk::host::ServerModule<S>> {
    Box::new(SportModule)
}
