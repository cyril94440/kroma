//! AllDebrid as a download engine: AllDebrid fetches the torrent on its own
//! servers, and this module answers the `download-client` point from its API.
//! Finished files are fetched one at a time into a cache that purges itself.

use std::path::Path;

use anyhow::Result;

pub mod api;
pub mod cache;
pub mod files;
pub mod port;
pub mod purge_job;
pub mod types;

pub use types::{ClientDef, TorrentState, TorrentStatus};

mod add;
mod lifecycle;
mod ready;
mod resolve;
mod status;

#[cfg(test)]
mod fake_alldebrid;
#[cfg(test)]
mod it_engine;

pub const KIND: &str = "alldebrid";
pub const MODULE_ID: &str = "tv.kroma.engine.alldebrid";
pub const DATA_DIR: &str = "alldebrid";

use kroma_module_sdk::EmbeddedModule;
pub const MODULE: EmbeddedModule = kroma_module_sdk::embedded_module!();

pub struct AllDebrid {
    api: api::Api,
    cache: cache::Cache,
    staging: std::path::PathBuf,
}

impl AllDebrid {
    /// The API key travels in `def.password`; `def.url` overrides the API base
    /// when set. `module_dir` is this module's own folder under the data dir.
    pub fn new(def: &ClientDef, module_dir: &Path) -> Result<Self> {
        Ok(Self {
            api: api::Api::new(&def.url, &def.password)?,
            cache: cache::Cache::new(module_dir),
            staging: module_dir.join("torrents"),
        })
    }

    pub fn test(&self) -> Result<String> {
        let user = self.api.user()?;
        if !user.is_premium {
            anyhow::bail!("the AllDebrid account {} is not premium", user.username);
        }
        Ok(format!("{} (premium)", user.username))
    }

    pub fn cache(&self) -> &cache::Cache {
        &self.cache
    }
}

pub(crate) fn ensure_magnet_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        anyhow::bail!("not an AllDebrid magnet id");
    }
    Ok(())
}

pub struct AllDebridModule;

#[kroma_module_sdk::host::async_trait]
impl<S: kroma_module_sdk::host::HostCtx + Clone + Send + Sync + 'static>
    kroma_module_sdk::host::ServerModule<S> for AllDebridModule
{
    fn id(&self) -> &'static str {
        MODULE_ID
    }

    fn jobs(&self) -> Vec<kroma_module_sdk::host::ModuleJob<S>> {
        vec![purge_job::job::<S>()]
    }
}

pub fn server_module<S: kroma_module_sdk::host::HostCtx + Clone + Send + Sync + 'static>(
) -> Box<dyn kroma_module_sdk::host::ServerModule<S>> {
    Box::new(AllDebridModule)
}
