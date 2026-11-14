//! Names a test can make "resolve" to a public-looking address while the bytes
//! really come from a plain-HTTP socket on this machine. Everything that vets a
//! request still runs on the name; only the last step, the address curl
//! connects to, is swapped. Compiled only with the `test-hosts` feature, which
//! no build of the server turns on.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Mutex, PoisonError};

struct Mapped {
    address: IpAddr,
    transport: SocketAddr,
}

static HOSTS: Mutex<Option<HashMap<String, Mapped>>> = Mutex::new(None);

/// `host` answers `address` to a lookup and is served from `transport`.
pub fn map_host(host: &str, address: IpAddr, transport: SocketAddr) {
    HOSTS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert_with(HashMap::new)
        .insert(host.to_ascii_lowercase(), Mapped { address, transport });
}

pub(crate) fn address_of(host: &str) -> Option<IpAddr> {
    let hosts = HOSTS.lock().unwrap_or_else(PoisonError::into_inner);
    hosts.as_ref()?.get(&host.to_ascii_lowercase()).map(|mapped| mapped.address)
}

/// `url` pointed at the socket its host is mapped to, as plain HTTP.
pub(crate) fn transport_url(url: &str) -> Option<String> {
    let target = crate::address::web_target(url)?;
    let hosts = HOSTS.lock().unwrap_or_else(PoisonError::into_inner);
    let mapped = hosts.as_ref()?.get(&target.host.to_ascii_lowercase())?;
    let rest = url.split_once("://")?.1;
    let path = rest.find(['/', '?']).map_or("", |at| &rest[at..]);
    Some(format!("http://{}{path}", mapped.transport))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mapped_name_keeps_its_path_and_query_on_the_new_socket() {
        map_host("map-test.example", "93.184.216.34".parse().unwrap(), "127.0.0.1:9".parse().unwrap());

        assert_eq!(
            transport_url("https://map-test.example/a/b.mkv?t=1").as_deref(),
            Some("http://127.0.0.1:9/a/b.mkv?t=1")
        );
        assert_eq!(transport_url("https://map-test.example").as_deref(), Some("http://127.0.0.1:9"));
        assert_eq!(address_of("MAP-TEST.example"), Some("93.184.216.34".parse().unwrap()));
        assert!(transport_url("https://other.example/a").is_none());
    }
}
