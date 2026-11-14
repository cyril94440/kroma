//! Where the bytes of one remote file are, for a caller that holds only the
//! file's reference: a local path when it is cached, otherwise a fresh URL.

use std::path::Path;
use std::time::Instant;

use anyhow::{anyhow, bail, Result};
use serde::Serialize;

use crate::api::{self, Api};
use crate::cache::Cache;
use crate::creds::{Creds, Saved};
use crate::files::Planned;
use crate::memo;
use crate::ready::parse_reference;
use crate::resolve::plan_with;
use crate::settings::CacheSettings;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Located {
    pub path: Option<String>,
    pub url: Option<String>,
    pub size: u64,
}

pub fn locate(module_dir: &Path, settings: CacheSettings, reference: &str, warm: bool) -> Result<Located> {
    let (magnet, rel) = parse_reference(reference).ok_or_else(|| anyhow!("not a reference this module made"))?;
    let saved = Creds::new(module_dir).candidates(magnet);
    let (file, _) = through(&saved, |api| {
        plan_with(api, magnet)?
            .into_iter()
            .find(|file| file.rel == rel)
            .ok_or_else(|| anyhow!("the magnet no longer has {rel}"))
    })?;
    let cache = Cache::new(module_dir);
    if let Some(path) = cache.cached_path(magnet, &file) {
        cache.touch(&path);
        let path = path.canonicalize().unwrap_or(path);
        return Ok(Located { path: Some(path.to_string_lossy().into_owned()), url: None, size: file.size });
    }
    let (url, _) = through(&saved, |api| url_of(api, magnet, &file))?;
    if warm && settings.enabled() {
        let ((), api) = through(&saved, |api| api.user().map(|_| ()))?;
        cache.ensure_cached(&api, magnet, &file)?;
    }
    Ok(Located { path: None, url: Some(url), size: file.size })
}

fn url_of(api: &Api, magnet: &str, file: &Planned) -> Result<String> {
    let key = format!("{}|{magnet}|{}", api.base(), file.rel);
    if let Some(known) = memo::urls().get(&key, Instant::now()) {
        return Ok(known);
    }
    let url = api.unlock(&file.link)?.link;
    memo::urls().put(key, url.clone(), Instant::now());
    Ok(url)
}

fn through<T>(saved: &[Saved], mut ask: impl FnMut(&Api) -> Result<T>) -> Result<(T, Api)> {
    if saved.is_empty() {
        bail!("no AllDebrid key is stored for this file: grab or test an AllDebrid client first");
    }
    let mut last = None;
    for each in saved {
        let api = Api::new(&each.url, &each.key)?;
        match ask(&api) {
            Ok(found) => return Ok((found, api)),
            Err(error) if is_auth(&error) => last = Some(error),
            Err(error) => return Err(error),
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("AllDebrid refused every stored key")))
}

fn is_auth(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<api::Refused>()
        .is_some_and(|refused| refused.code.starts_with("AUTH_"))
}
