//! What an add can be handed, and how each reaches AllDebrid.


use anyhow::{bail, Context, Result};
use kroma_module_sdk::http::address::web_target;
use kroma_module_sdk::http::Fetch;

use crate::AllDebrid;

const TORRENT_PURPOSE: &str = "torrent-file";
const TORRENT_SECS: u32 = 30;
const MAX_TORRENT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum Link<'a> {
    Magnet(&'a str),
    Web(&'a str),
}

fn classify(link: &str) -> Option<Link<'_>> {
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
        32 => text.bytes().all(|b| b.is_ascii_uppercase() || (b'2'..=b'7').contains(&b)),
        _ => false,
    }
}

impl AllDebrid {
    pub fn add(&self, magnet_or_url: &str) -> Result<String> {
        let Some(link) = classify(magnet_or_url) else {
            bail!("AllDebrid takes a magnet link, an info hash or a link to a .torrent file");
        };
        match link {
            Link::Magnet(magnet) => self.api.upload_magnet(magnet),
            Link::Web(url) => self.upload_fetched(url),
        }
    }

    fn upload_fetched(&self, url: &str) -> Result<String> {
        let bytes = fetch_torrent(url)?;
        std::fs::create_dir_all(&self.staging).context("create the torrent staging folder")?;
        let file = self
            .staging
            .join(format!("{}.torrent", kroma_module_sdk::primitives::random_token()));
        std::fs::write(&file, &bytes).context("stage the .torrent file")?;
        let uploaded = self.api.upload_torrent(&file);
        let _ = std::fs::remove_file(&file);
        uploaded
    }
}

fn fetch_torrent(url: &str) -> Result<Vec<u8>> {
    let host = web_target(url).map(|target| target.host).unwrap_or_default();
    let response = Fetch::new()
        .purpose(TORRENT_PURPOSE)
        .max_time(TORRENT_SECS)
        .max_bytes(MAX_TORRENT_BYTES)
        .get(url)
        .with_context(|| format!("fetch the .torrent from {host}"))?;
    if !(200..300).contains(&response.status) {
        bail!("{host} answered HTTP {} for the .torrent file", response.status);
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

        assert_eq!(classify("magnet:?xt=urn:btih:AB"), Some(Link::Magnet("magnet:?xt=urn:btih:AB")));
        assert_eq!(classify(hash), Some(Link::Magnet(hash)));
        assert_eq!(classify(" HTTPS://t/x.torrent "), Some(Link::Web("HTTPS://t/x.torrent")));
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

    #[test]
    fn a_staged_file_name_carries_no_path_from_the_link() {
        let staging = std::path::Path::new("/stage");
        let name = format!("{}.torrent", kroma_module_sdk::primitives::random_token());

        assert_eq!(std::path::Path::new(&name).components().count(), 1);
        assert!(staging.join(&name).starts_with(staging));
    }
}
