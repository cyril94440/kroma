use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kroma_http::address::{is_public, WebTarget};
use kroma_http::egress::{self, Exchange, Outgoing};
use kroma_module_manifest::{Attempt, Permissions, Proto};
use kroma_primitives::input::{MediaInput, RemoteUrl};
use kroma_primitives::remote_file::RemoteFile;

use crate::services::network::DeclaredPermissions;

use super::point::Located;
use super::{Refusal, SourceError};

pub(super) const PURPOSE: &str = "playback";

/// The folders a module may hold a title in: where each module's own data
/// directories sit, by module id.
pub type SourceRoots = Arc<dyn Fn(&str) -> Vec<PathBuf> + Send + Sync>;

pub(super) struct Vetting {
    pub(super) declared: DeclaredPermissions,
    pub(super) roots: SourceRoots,
    #[cfg(feature = "insecure-test-sources")]
    pub(super) insecure: std::sync::atomic::AtomicBool,
}

impl Vetting {
    pub(super) fn new(declared: DeclaredPermissions, roots: SourceRoots) -> Self {
        Self {
            declared,
            roots,
            #[cfg(feature = "insecure-test-sources")]
            insecure: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(super) fn vet(&self, located: &Located, stub: &RemoteFile) -> Result<MediaInput, SourceError> {
        if let Some(path) = &located.path {
            return self.vet_path(&located.module_id, path, stub).map(MediaInput::Local);
        }
        match &located.url {
            Some(url) => self.vet_url(&located.module_id, url).map(MediaInput::Remote),
            None => Err(SourceError::Refused(Refusal::NoPathNoUrl)),
        }
    }

    fn vet_path(&self, module: &str, path: &str, stub: &RemoteFile) -> Result<PathBuf, SourceError> {
        let refused = || SourceError::Refused(Refusal::PathOutsideItsFolders);
        if !Path::new(path).is_absolute() {
            return Err(refused());
        }
        let canonical = Path::new(path).canonicalize().map_err(|_| refused())?;
        let held = (self.roots)(module)
            .iter()
            .filter_map(|root| root.canonicalize().ok())
            .any(|root| canonical.starts_with(root));
        if !held {
            return Err(refused());
        }
        let complete = std::fs::metadata(&canonical)
            .is_ok_and(|meta| meta.is_file() && meta.len() == stub.size);
        if !complete {
            return Err(SourceError::Refused(Refusal::NotTheFileStubbed));
        }
        Ok(canonical)
    }

    fn vet_url(&self, module: &str, url: &str) -> Result<RemoteUrl, SourceError> {
        let parsed = self.parse(url).map_err(|why| SourceError::Refused(Refusal::Url(why)))?;
        let target = kroma_http::address::web_target(parsed.expose())
            .ok_or(SourceError::Refused(Refusal::Url(kroma_primitives::input::RemoteUrlError::NotAUrl)))?;
        let (exchange, refusal) = Exchange::direct_only(
            PURPOSE,
            "GET",
            target.host.clone(),
            target.port,
            egress::decide(&Outgoing {
                purpose: PURPOSE,
                method: "GET",
                target: target.clone(),
            }),
            "ffmpeg cannot take a network",
        );
        egress::record(exchange);
        if refusal.is_some() {
            return Err(SourceError::Refused(Refusal::NetworkPolicy));
        }
        if self.is_insecure() {
            return Ok(parsed);
        }
        if let Some(permissions) = (self.declared)(module) {
            if !declares_host(&permissions, &target) {
                return Err(SourceError::Refused(Refusal::HostNotDeclared));
            }
        }
        if !resolves_public(&target) {
            return Err(SourceError::Refused(Refusal::HostResolvesPrivate));
        }
        Ok(parsed)
    }

    #[cfg(feature = "insecure-test-sources")]
    fn parse(&self, url: &str) -> Result<RemoteUrl, kroma_primitives::input::RemoteUrlError> {
        if self.is_insecure() {
            RemoteUrl::parse_insecure(url)
        } else {
            RemoteUrl::parse(url)
        }
    }

    #[cfg(not(feature = "insecure-test-sources"))]
    fn parse(&self, url: &str) -> Result<RemoteUrl, kroma_primitives::input::RemoteUrlError> {
        RemoteUrl::parse(url)
    }

    #[cfg(feature = "insecure-test-sources")]
    pub(super) fn is_insecure(&self) -> bool {
        self.insecure.load(std::sync::atomic::Ordering::SeqCst)
    }

    #[cfg(not(feature = "insecure-test-sources"))]
    pub(super) fn is_insecure(&self) -> bool {
        false
    }
}

fn declares_host(permissions: &Permissions, target: &WebTarget) -> bool {
    permissions.net.connect.iter().any(|entry| {
        let attempt = Attempt {
            purpose: &entry.purpose,
            host: &target.host,
            host_is_local: target.is_local(),
            port: target.port,
            proto: Proto::Tcp,
        };
        permissions.connect_reach(&attempt, &|_| Vec::new()).is_some()
    })
}

pub(super) fn resolves_public(target: &WebTarget) -> bool {
    if let Some(literal) = target.literal() {
        return is_public(literal);
    }
    let Ok(addresses) = (target.host.as_str(), target.port).to_socket_addrs() else {
        return false;
    };
    let mut any = false;
    for address in addresses {
        if !is_public(address.ip()) {
            return false;
        }
        any = true;
    }
    any
}
