use anyhow::{bail, Result};

use crate::AllDebrid;

impl AllDebrid {
    pub fn pause(&self, _client_ref: &str) -> Result<()> {
        bail!("AllDebrid cannot pause a download")
    }

    pub fn resume(&self, _client_ref: &str) -> Result<()> {
        bail!("AllDebrid cannot resume a download")
    }

    pub fn reannounce(&self, _client_ref: &str) -> Result<()> {
        Ok(())
    }

    pub fn remove(&self, client_ref: &str, _delete_data: bool) -> Result<()> {
        crate::ensure_magnet_id(client_ref)?;
        self.cache.evict_magnet(client_ref)?;
        self.api.delete(client_ref)
    }
}
