//! The URL a source answers for a title's bytes, vetted before anything fetches it.

use std::fmt;

use crate::address::{is_public, web_target};

const MAX_URL_BYTES: usize = 8 * 1024;

/// An `https` URL a module handed back for a title's bytes. Holding one is
/// proof that its scheme is `https`, that it carries no credentials, and that its
/// host is a public name or a public address: the URL is a bearer link, so its
/// path and query never appear in `Debug` or in a log line. A name that
/// resolves to a private address is the caller's to check, because only it can
/// resolve.
#[derive(Clone, PartialEq, Eq)]
pub struct RemoteUrl {
    url: String,
    host: String,
    pub(crate) https: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteUrlError {
    NotAUrl,
    NotHttps,
    PrivateHost,
}

impl fmt::Display for RemoteUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAUrl => "the source returned something that is not a plain URL",
            Self::NotHttps => "the source returned a URL that is not https",
            Self::PrivateHost => "the source returned a URL that is not on the public internet",
        })
    }
}

impl std::error::Error for RemoteUrlError {}

impl fmt::Debug for RemoteUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RemoteUrl({})", self.host)
    }
}

impl RemoteUrl {
    pub fn parse(url: &str) -> Result<Self, RemoteUrlError> {
        Self::checked(url, false)
    }

    /// Like [`Self::parse`] but takes plain `http` and loopback addresses, so an
    /// integration test can serve a title from a socket on this machine.
    #[cfg(feature = "insecure-test-urls")]
    pub fn parse_insecure(url: &str) -> Result<Self, RemoteUrlError> {
        Self::checked(url, true)
    }

    fn checked(url: &str, insecure: bool) -> Result<Self, RemoteUrlError> {
        if url.is_empty()
            || url.len() > MAX_URL_BYTES
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(RemoteUrlError::NotAUrl);
        }
        let target = web_target(url).ok_or(RemoteUrlError::NotAUrl)?;
        if !target.https && !insecure {
            return Err(RemoteUrlError::NotHttps);
        }
        if !insecure && (target.is_local() || !names_a_public_host(&target.host)) {
            return Err(RemoteUrlError::PrivateHost);
        }
        Ok(Self {
            url: url.to_string(),
            host: target.host,
            https: target.https,
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    /// The whole URL. It is a bearer link: hand it to the one process that
    /// fetches it and to nothing that logs.
    pub fn expose(&self) -> &str {
        &self.url
    }

    pub fn redact(&self, text: &str) -> String {
        text.replace(&self.url, &self.host)
    }
}

fn names_a_public_host(host: &str) -> bool {
    if let Ok(ip) = host.parse() {
        return is_public(ip);
    }
    let last = host.trim_end_matches('.').rsplit('.').next().unwrap_or_default();
    last.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MediaInput;

    #[test]
    fn a_public_https_url_is_accepted() {
        let url = RemoteUrl::parse("https://cdn.example.com/dl/a.mkv?token=abc").unwrap();

        assert_eq!(url.host(), "cdn.example.com");
        assert!(RemoteUrl::parse("https://1.1.1.1/a.mkv").is_ok());
        assert!(RemoteUrl::parse("https://[2606:4700::1111]/a.mkv").is_ok());
    }

    #[test]
    fn anything_but_https_is_refused() {
        for url in ["http://cdn.example.com/a", "file:///etc/passwd", "ftp://x.example.com/a", "data:text/plain,hi", "concat:a|b"] {
            assert!(RemoteUrl::parse(url).is_err(), "{url}");
        }
        assert_eq!(RemoteUrl::parse("http://cdn.example.com/a"), Err(RemoteUrlError::NotHttps));
    }

    #[test]
    fn a_host_off_the_public_internet_is_refused() {
        let refused = [
            "https://localhost/a",
            "https://LOCALHOST./a",
            "https://127.0.0.1/a",
            "https://10.0.0.5/a",
            "https://192.168.1.10:8443/a",
            "https://172.16.0.1/a",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/a",
            "https://[fe80::1]/a",
            "https://[::ffff:127.0.0.1]/a",
            "https://0.0.0.0/a",
            "https://nas/a",
            "https://printer.local/a",
            "https://box.internal/a",
            "https://0x7f.0.0.1/a",
            "https://2130706433/a",
            "https://127.1/a",
        ];
        for url in refused {
            assert!(RemoteUrl::parse(url).is_err(), "{url}");
        }
    }

    #[test]
    fn a_url_that_hides_its_destination_is_refused() {
        for url in ["https://cdn.example.com@127.0.0.1/a", "https://user:pw@cdn.example.com/a", "", "https://", "https://cdn.example.com/a b", "https://cdn.example.com/a\nb", "not a url"] {
            assert!(RemoteUrl::parse(url).is_err(), "{url:?}");
        }
    }

    #[test]
    fn debug_never_shows_the_path_or_the_query() {
        let url = RemoteUrl::parse("https://cdn.example.com/secret/a.mkv?token=hunter2").unwrap();

        let shown = format!("{url:?} {:?}", MediaInput::Remote(url.clone()));

        assert!(!shown.contains("hunter2") && !shown.contains("secret"), "{shown}");
        assert!(shown.contains("cdn.example.com"));
    }

    #[test]
    fn redact_swaps_the_whole_url_for_its_host() {
        let url = RemoteUrl::parse("https://cdn.example.com/secret/a.mkv?token=hunter2").unwrap();

        let said = MediaInput::Remote(url).redact("https://cdn.example.com/secret/a.mkv?token=hunter2: Server returned 403");

        assert_eq!(said, "cdn.example.com: Server returned 403");
    }
}
