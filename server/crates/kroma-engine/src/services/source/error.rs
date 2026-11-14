use std::fmt;

use kroma_primitives::remote_url::RemoteUrlError;

/// Why a title's bytes could not be reached. None of these carries a URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    BadStub(String),
    NoModule,
    Unreachable,
    Gone(String),
    Refused(Refusal),
}

/// A source's answer the core would not use, and which rule it broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Url(RemoteUrlError),
    HostNotDeclared,
    HostResolvesPrivate,
    PathOutsideItsFolders,
    NotTheFileStubbed,
    NoPathNoUrl,
    NetworkPolicy,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(why) => write!(f, "{why}"),
            Self::HostNotDeclared => f.write_str("the URL's host is not one the source declared it connects to"),
            Self::HostResolvesPrivate => f.write_str("the URL's host resolves to an address off the public internet"),
            Self::PathOutsideItsFolders => f.write_str("the path is outside the folders the source may hold"),
            Self::NotTheFileStubbed => f.write_str("the path is not a complete file of the size the stub declares"),
            Self::NoPathNoUrl => f.write_str("the source answered neither a path nor a URL"),
            Self::NetworkPolicy => f.write_str("the network policy does not let a title be read from this host directly"),
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadStub(why) => write!(f, "the stub is not readable: {why}"),
            Self::NoModule => f.write_str("no module answers for this title's source"),
            Self::Unreachable => f.write_str("the source did not answer"),
            Self::Gone(why) => write!(f, "the source could not locate the file: {why}"),
            Self::Refused(rule) => write!(f, "the source's answer was refused: {rule}"),
        }
    }
}

impl std::error::Error for SourceError {}

impl SourceError {
    /// Whether asking again later can succeed: a module that is off or not
    /// answering comes back, a refused answer or an unreadable stub does not.
    pub fn is_transient(&self) -> bool {
        matches!(self, Self::NoModule | Self::Unreachable | Self::Gone(_))
    }
}
