#![allow(clippy::result_large_err)]

#[cfg(target_family = "wasm")]
extern crate kroma_sqlite_guest as rusqlite;

mod chat_rules;
mod clock;
mod live;
mod model;
mod module;
mod party;
mod poll;
mod room;
mod routes;
mod store;
mod strings;
mod sync;
mod vote;
mod voice;
mod votewords;

pub use clock::{system_clock, Clock};
pub use live::{Live, PeerLive, CONTROL};
pub use module::MODULE;
pub use party::{Actor, Party, Request, SwitchResult};
pub use room::ROOM;
pub use routes::{point_routes, routes};

pub const MODULE_ID: &str = "tv.kroma.party";

pub struct PartyModule;

fn archive_job<S: kroma_module_sdk::host::HostCtx>(host: &S) -> anyhow::Result<()> {
    if let Some(party) = kroma_module_sdk::host::service::<Party>(host) {
        party.archive()?;
    }
    Ok(())
}

#[kroma_module_sdk::host::async_trait]
impl<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static>
    kroma_module_sdk::host::ServerModule<S> for PartyModule
{
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn migrations(&self) -> &'static str {
        store::MIGRATIONS
    }

    fn admin_routes(&self, host: &S) -> Option<axum::Router<S>> {
        kroma_module_sdk::host::service::<Party>(host).map(routes::<S>)
    }

    fn jobs(&self) -> Vec<kroma_module_sdk::host::ModuleJob<S>> {
        vec![kroma_module_sdk::host::ModuleJob {
            key: "party.archive",
            category: "maintenance",
            schedule: Some("0 4 * * *"),
            run: archive_job::<S>,
        }]
    }

    fn events(&self) -> Vec<&'static str> {
        vec!["live.proposal"]
    }

    async fn on_event(&self, host: S, topic: String, payload: serde_json::Value) {
        if topic != "live.proposal" {
            return;
        }
        if let Some(party) = kroma_module_sdk::host::service::<Party>(&host) {
            let _ = kroma_module_sdk::host::run_blocking(move || party.on_proposal(&payload)).await;
        }
    }

    async fn on_enable(&self, host: S) {
        if let Some(party) = kroma_module_sdk::host::service::<Party>(&host) {
            party.start();
        }
    }

    async fn on_disable(&self, host: S) {
        if let Some(party) = kroma_module_sdk::host::service::<Party>(&host) {
            party.stop();
        }
    }
}

pub fn server_module<S: kroma_module_sdk::host::HostStorage + Clone + Send + Sync + 'static>(
) -> Box<dyn kroma_module_sdk::host::ServerModule<S>> {
    Box::new(PartyModule)
}
