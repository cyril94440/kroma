use std::time::Instant;

use anyhow::{anyhow, Result};

use crate::api::{self, Api};
use crate::cache::Availability;
use crate::files::{self, Planned};
use crate::memo;
use crate::AllDebrid;

/// The files of a ready magnet, flattened and made safe, remembered for a few
/// minutes.
pub fn plan_with(api: &Api, magnet: &str) -> Result<Vec<Planned>> {
    crate::ensure_magnet_id(magnet)?;
    let key = format!("{}|{magnet}", api.base());
    if let Some(known) = memo::plans().get(&key, Instant::now()) {
        return Ok(known);
    }
    let listed = api.files(magnet).map_err(|error| {
        if api::is_unknown_magnet(&error) {
            files::unusable("this magnet is no longer on AllDebrid")
        } else {
            error
        }
    })?;
    let plan = files::flatten(&listed)?;
    memo::plans().put(key, plan.clone(), Instant::now());
    Ok(plan)
}

/// A plan failure that no retry will cure: the magnet's own contents, or an
/// answer AllDebrid gave about them. Transport, rate limits and a rejected key
/// are not.
pub fn is_unusable(error: &anyhow::Error) -> bool {
    error.is::<files::Unusable>()
        || error.downcast_ref::<api::Refused>().is_some_and(|refused| !refused.is_auth())
}

impl AllDebrid {
    pub fn list_files(&self, magnet: &str) -> Result<Vec<Planned>> {
        plan_with(&self.api, magnet)
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

    pub fn cache_file(&self, magnet: &str, rel: &str, limits: crate::cache::Limits) -> Result<Availability> {
        let file = self.find(magnet, rel)?;
        self.cache.ensure_cached(&self.api, magnet, &file, limits)
    }
}
