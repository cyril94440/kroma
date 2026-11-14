use anyhow::{anyhow, Result};

use crate::cache::Availability;
use crate::files::{self, Planned};
use crate::AllDebrid;

impl AllDebrid {
    pub fn list_files(&self, magnet: &str) -> Result<Vec<Planned>> {
        crate::ensure_magnet_id(magnet)?;
        files::flatten(&self.api.files(magnet)?)
    }

    fn find(&self, magnet: &str, rel: &str) -> Result<Planned> {
        self.list_files(magnet)?
            .into_iter()
            .find(|file| file.rel == rel)
            .ok_or_else(|| anyhow!("the magnet has no file {rel}"))
    }

    /// A fresh unlocked URL for one file. AllDebrid's links expire, so this is
    /// asked for each time one is needed.
    pub fn direct_url(&self, magnet: &str, rel: &str) -> Result<String> {
        Ok(self.api.unlock(&self.find(magnet, rel)?.link)?.link)
    }

    pub fn cache_file(&self, magnet: &str, rel: &str) -> Result<Availability> {
        let file = self.find(magnet, rel)?;
        self.cache.ensure_cached(&self.api, magnet, &file)
    }
}
