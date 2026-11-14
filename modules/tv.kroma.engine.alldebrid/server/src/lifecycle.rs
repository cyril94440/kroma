use anyhow::{bail, Result};

use crate::cache::Cache;
use crate::AllDebrid;

const READY: u8 = 4;

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

    /// A magnet that has not finished is deleted on AllDebrid. A finished one is
    /// kept there, because the library streams from it; `delete_data` only
    /// removes the stubs and sidecar files saved here.
    pub fn remove(&self, client_ref: &str, delete_data: bool) -> Result<()> {
        crate::ensure_magnet_id(client_ref)?;
        if delete_data {
            let folder = self.stubs_folder(client_ref);
            Cache::stop_fetching_under(&folder);
            match std::fs::remove_dir_all(&folder) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(anyhow::Error::new(error).context("delete the saved files"));
                }
                _ => {}
            }
        }
        match self.api.magnet(client_ref)? {
            Some(magnet) if magnet.status_code == READY => Ok(()),
            Some(_) => self.api.delete(client_ref),
            None => Ok(()),
        }
    }
}
