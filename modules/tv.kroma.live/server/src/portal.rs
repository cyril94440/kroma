use std::fmt;

use anyhow::{bail, Result};

#[derive(Clone, PartialEq, Eq)]
pub struct Portal {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Portal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Portal({}://{}:{})", self.scheme, self.host, self.port)
    }
}

fn query_value(query: &str, name: &str) -> String {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| decode(value))
        .unwrap_or_default()
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' if at + 2 < bytes.len() => {
                match u8::from_str_radix(&value[at + 1..at + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        at += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        at += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                at += 1;
            }
            other => {
                out.push(other);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

impl Portal {
    pub fn parse(url: &str) -> Result<Portal> {
        let url = url.trim();
        let (scheme, rest) = url.split_once("://").unwrap_or(("http", url));
        if scheme != "http" && scheme != "https" {
            bail!("a portal URL starts with http:// or https://");
        }
        let (authority, tail) = rest.split_once('/').map_or((rest, ""), |(a, t)| (a, t));
        let query = tail.split_once('?').map_or("", |(_, q)| q);
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) if port.bytes().all(|b| b.is_ascii_digit()) && !port.is_empty() => (host, port.parse()?),
            _ => (authority, if scheme == "https" { 443 } else { 80 }),
        };
        if host.is_empty() {
            bail!("a portal URL names a host");
        }
        Ok(Portal {
            scheme: scheme.to_string(),
            host: host.to_string(),
            port,
            username: query_value(query, "username"),
            password: query_value(query, "password"),
        })
    }

    fn base(&self) -> String {
        format!("{}://{}:{}", self.scheme, self.host, self.port)
    }

    pub fn api_url(&self, action: Option<&str>) -> String {
        let credentials = format!("username={}&password={}", encode(&self.username), encode(&self.password));
        match action {
            Some(action) => format!("{}/player_api.php?{credentials}&action={action}", self.base()),
            None => format!("{}/player_api.php?{credentials}", self.base()),
        }
    }

    pub fn xmltv_url(&self) -> String {
        format!(
            "{}/xmltv.php?username={}&password={}",
            self.base(),
            encode(&self.username),
            encode(&self.password)
        )
    }

    pub fn stream_url(&self, stream_id: i64) -> String {
        format!("{}/{}/{}/{stream_id}.ts", self.base(), self.username, self.password)
    }

    pub fn owns(&self, url: &str) -> bool {
        url.contains(&format!("//{}:{}/{}/", self.host, self.port, self.username))
    }
}

pub fn is_playlist_file(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or("").to_lowercase();
    path.ends_with(".m3u") || path.ends_with(".m3u8")
}

pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = ["http://", "https://", "rtmp://", "rtsp://"].iter().filter_map(|p| rest.find(p)).min() {
        out.push_str(&rest[..at]);
        out.push_str("<url>");
        let tail = &rest[at..];
        let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PORTAL: &str = "http://portal.test:8080/get.php?username=TESTUSER&password=TESTPASS42&type=m3u";

    #[test]
    fn a_portal_url_gives_the_api_the_stream_and_the_guide() {
        let portal = Portal::parse(PORTAL).unwrap();

        assert_eq!(portal.api_url(Some("get_live_streams")), "http://portal.test:8080/player_api.php?username=TESTUSER&password=TESTPASS42&action=get_live_streams");
        assert_eq!(portal.stream_url(77), "http://portal.test:8080/TESTUSER/TESTPASS42/77.ts");
        assert_eq!(portal.xmltv_url(), "http://portal.test:8080/xmltv.php?username=TESTUSER&password=TESTPASS42");
        assert!(portal.owns("http://portal.test:8080/TESTUSER/TESTPASS42/1.ts"));
        assert!(!portal.owns("http://portal.test:8080/OTHER/x/1.ts"));
    }

    #[test]
    fn a_port_defaults_to_the_schemes_and_a_bad_scheme_is_refused() {
        assert_eq!(Portal::parse("https://p.test/x?username=a&password=b").unwrap().port, 443);
        assert_eq!(Portal::parse("p.test/x?username=a&password=b").unwrap().port, 80);
        assert!(Portal::parse("ftp://p.test/").is_err());
    }

    #[test]
    fn a_portal_never_prints_its_credentials() {
        let portal = Portal::parse(PORTAL).unwrap();

        let shown = format!("{portal:?}");

        assert!(!shown.contains("TESTUSER") && !shown.contains("TESTPASS42"), "{shown}");
    }

    #[test]
    fn urls_are_replaced_before_any_text_is_kept() {
        let text = "Server returned 403 for http://portal.test:8080/TESTUSER/TESTPASS42/1.ts: denied";

        assert_eq!(redact(text), "Server returned 403 for <url> denied");
    }

    #[test]
    fn a_playlist_file_is_told_from_a_portal() {
        assert!(is_playlist_file("http://x/list.m3u8?token=1"));
        assert!(!is_playlist_file("http://x/get.php?username=a&password=b&type=m3u"));
        assert!(!is_playlist_file(PORTAL.replace("&type=m3u", "").as_str()));
    }
}
