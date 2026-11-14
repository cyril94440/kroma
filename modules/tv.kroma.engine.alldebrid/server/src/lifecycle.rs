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

    /// A magnet this module added that has not finished is deleted on
    /// AllDebrid, and so is a finished one when `discard` says its release was
    /// turned down. Any other finished magnet stays, because the library
    /// streams from it, and a magnet the user added themselves is never touched.
    /// `delete_data` removes the stubs and sidecar files saved here.
    pub fn remove(&self, client_ref: &str, delete_data: bool, discard: bool) -> Result<()> {
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
        if !self.owned.has(client_ref) {
            return Ok(());
        }
        let forget = || {
            self.owned.release(client_ref);
            self.creds.forget(client_ref);
        };
        match self.api.magnet(client_ref)? {
            Some(magnet) if magnet.status_code == READY && !discard => Ok(()),
            Some(_) => {
                self.api.delete(client_ref)?;
                forget();
                Ok(())
            }
            None => {
                forget();
                Ok(())
            }
        }
    }
}
