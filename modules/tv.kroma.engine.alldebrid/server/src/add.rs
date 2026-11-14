//! What an add can be handed, and how each reaches AllDebrid.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use kroma_module_sdk::http::address::web_target;
use kroma_module_sdk::http::Fetch;

use crate::AllDebrid;

const TORRENT_PURPOSE: &str = "torrent-file";
pub(crate) const TORRENT_SECS: u32 = 30;
const MAX_TORRENT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Link<'a> {
    Magnet(&'a str),
    Web(&'a str),
}

pub(crate) fn classify(link: &str) -> Option<Link<'_>> {
    let link = link.trim();
    if link.starts_with("magnet:") || is_info_hash(link) {
        return Some(Link::Magnet(link));
    }
    let lower = link.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://")).then_some(Link::Web(link))
}

fn is_info_hash(text: &str) -> bool {
    match text.len() {
        40 => text.bytes().all(|b| b.is_ascii_hexdigit()),
        32 => text
            .bytes()
            .all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)),
        _ => false,
    }
}

impl AllDebrid {
    pub fn add(&self, magnet_or_url: &str) -> Result<String> {
        let Some(link) = classify(magnet_or_url) else {
            bail!("AllDebrid takes a magnet link, an info hash or a link to a .torrent file");
        };
        let before = self.snapshot();
        let id = match link {
            Link::Magnet(magnet) => self.api.upload_magnet(magnet)?,
            Link::Web(url) => self.upload_fetched(url, Duration::from_secs(u64::from(TORRENT_SECS)))?.id,
        };
        self.claim_if_new(before.as_ref(), &id);
        self.remember(Some(&id));
        Ok(id)
    }

    pub(crate) fn snapshot(&self) -> Option<std::collections::HashSet<String>> {
        match self.api.magnet_ids() {
            Ok(ids) => Some(ids),
            Err(error) => {
                tracing::warn!("could not list the account's magnets; none will be deleted: {error:#}");
                None
            }
        }
    }

    pub(crate) fn claim_if_new(&self, before: Option<&std::collections::HashSet<String>>, id: &str) {
        if crate::owned::is_new(before, id) {
            self.owned.claim(id);
        }
    }

    pub(crate) fn upload_fetched(&self, url: &str, left: Duration) -> Result<crate::api::Uploaded> {
        let bytes = fetch_torrent(url, left)?;
        let staging = self.module_dir.join("torrents");
        std::fs::create_dir_all(&staging).context("create the torrent staging folder")?;
        let file = staging.join(format!(
            "{}.torrent",
            kroma_module_sdk::primitives::random_token()
        ));
        std::fs::write(&file, &bytes).context("stage the .torrent file")?;
        let uploaded = self.api.within(left).upload_torrent_entry(&file);
        let _ = std::fs::remove_file(&file);
        uploaded
    }
}

fn fetch_torrent(url: &str, left: Duration) -> Result<Vec<u8>> {
    let host = web_target(url)
        .map(|target| target.host)
        .unwrap_or_default();
    let response = Fetch::new()
        .purpose(TORRENT_PURPOSE)
        .max_time((left.as_secs() as u32).clamp(1, TORRENT_SECS))
        .max_bytes(MAX_TORRENT_BYTES)
        .get(url)
        .with_context(|| format!("fetch the .torrent from {host}"))?;
    if !(200..300).contains(&response.status) {
        bail!(
            "{host} answered HTTP {} for the .torrent file",
            response.status
        );
    }
    if response.body.first() != Some(&b'd') {
        bail!("{host} did not return a .torrent file");
    }
    Ok(response.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_magnet_a_hash_and_a_web_link_are_told_apart() {
        let hash = "0123456789abcdef0123456789abcdef01234567";

        assert_eq!(
            classify("magnet:?xt=urn:btih:AB"),
            Some(Link::Magnet("magnet:?xt=urn:btih:AB"))
        );
        assert_eq!(classify(hash), Some(Link::Magnet(hash)));
        assert_eq!(
            classify(" HTTPS://t/x.torrent "),
            Some(Link::Web("HTTPS://t/x.torrent"))
        );
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(classify("file:///etc/passwd"), None);
        assert_eq!(classify("ftp://x/y.torrent"), None);
        assert_eq!(classify("abc"), None);
        assert_eq!(classify(""), None);
    }

    #[test]
    fn a_base32_info_hash_is_a_magnet() {
        assert!(is_info_hash("ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"));
        assert!(!is_info_hash("ABCDEFGHIJKLMNOPQRSTUVWXYZ234561"));
    }
}
