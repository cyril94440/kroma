use kroma_module_sdk::http::address::web_target;

const DECLARED: &[&str] = &["*.debrid.it", "*.alldeb.ovh", "*.alldebrid.com"];

pub(crate) fn covered(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    DECLARED.iter().any(|pattern| match pattern.strip_prefix("*.") {
        Some(suffix) => host.strip_suffix(suffix).is_some_and(|rest| rest.ends_with('.')),
        None => host == *pattern,
    })
}

pub(crate) fn warn_if_unlisted(link: &str) {
    let Some(host) = web_target(link).map(|target| target.host) else {
        return;
    };
    if !covered(&host) {
        tracing::warn!(
            host = %host,
            "AllDebrid returned a link on {host}, which module.json does not allow under debrid-transfer: add it to net.connect"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subdomain_is_covered_and_the_bare_domain_is_not() {
        assert!(covered("abc.debrid.it"));
        assert!(covered("x.y.alldeb.ovh"));
        assert!(!covered("alldebrid.com"));
        assert!(!covered("evilalldebrid.com"));
        assert!(!covered("cdn.example.org"));
    }

    #[test]
    fn the_hosts_checked_are_the_ones_the_manifest_declares() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../module.json")).unwrap();
        let declared: Vec<String> = manifest["permissions"]["net"]["connect"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|entry| entry["purpose"] == "debrid-transfer")
            .flat_map(|entry| entry["to"].as_array().unwrap().clone())
            .map(|host| host.as_str().unwrap().to_string())
            .collect();

        assert_eq!(declared, DECLARED);
    }
}
