#![allow(clippy::result_large_err)]

#[cfg(target_family = "wasm")]
extern crate kroma_sqlite_guest as rusqlite;

mod agent;
mod blackbox;
mod clock;
mod model;
mod module;
mod paris;
mod peers;
mod prompt;
mod routes;
mod rules;
mod settings;
mod store;
mod strings;
mod tools;
mod votewords;
mod webcache;

use std::sync::Arc;

use kroma_module_sdk::host::{service, HostCtx};

pub use agent::Agent;
pub use clock::{system_clock, Clock};
pub use module::MODULE;
pub use peers::{CONTROL, ROOM, TOOLS};
pub use routes::routes;

pub const MODULE_ID: &str = "tv.kroma.hermes";

const TOPICS: [&str; 5] = ["party.message", "party.vote", "party.poll", "live.status", "live.failover"];

/// Builds the assistant over the host's peers and its own storage.
pub fn build(host: Arc<dyn HostCtx>, pool: store::Pool) -> Arc<Agent> {
    let source_host = host.clone();
    let models: model::ModelSource = Arc::new(move || model::model_from(&settings::Settings::read(&*source_host)));
    let deps = agent::Deps::new(
        host.clone(),
        Arc::new(peers::PeerRoom::new(host.clone())),
        Arc::new(peers::PeerControl::new(host.clone())),
        Arc::new(peers::PeerContributors::new(host)),
        store::Store::new(pool),
        system_clock(),
        models,
    );
    Agent::new(Arc::new(deps))
}

pub struct HermesModule;

fn maintenance_job<S: HostCtx>(host: &S) -> anyhow::Result<()> {
    if let Some(agent) = service::<Agent>(host) {
        agent.maintain()?;
    }
    Ok(())
}

#[kroma_module_sdk::host::async_trait]
impl<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static> kroma_module_sdk::host::ServerModule<S> for HermesModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn migrations(&self) -> &'static str {
        store::MIGRATIONS
    }

    fn admin_routes(&self, host: &S) -> Option<axum::Router<S>> {
        service::<Agent>(host).map(routes::<S>)
    }

    fn jobs(&self) -> Vec<kroma_module_sdk::host::ModuleJob<S>> {
        vec![kroma_module_sdk::host::ModuleJob { key: "hermes.maintenance", category: "maintenance", schedule: Some("30 4 * * *"), run: maintenance_job::<S> }]
    }

    fn events(&self) -> Vec<&'static str> {
        TOPICS.to_vec()
    }

    async fn on_event(&self, host: S, topic: String, payload: serde_json::Value) {
        if let Some(agent) = service::<Agent>(&host) {
            let _ = kroma_module_sdk::host::run_blocking(move || agent.on_event(&topic, &payload)).await;
        }
    }

    async fn on_enable(&self, host: S) {
        if let Some(agent) = service::<Agent>(&host) {
            agent.start();
        }
    }

    async fn on_disable(&self, host: S) {
        if let Some(agent) = service::<Agent>(&host) {
            agent.stop();
        }
    }
}

pub fn server_module<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static>() -> Box<dyn kroma_module_sdk::host::ServerModule<S>> {
    Box::new(HermesModule)
}

#[cfg(test)]
mod testing;
