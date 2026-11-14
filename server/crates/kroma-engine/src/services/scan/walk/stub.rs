//! What a library file that is a stub stands for.

use std::path::{Path, PathBuf};

use tracing::warn;

use kroma_primitives::remote_file;

use super::has_video_extension;

pub(super) struct StubFacts {
    pub(super) container: String,
    pub(super) size: u64,
}

pub(super) fn readable_stub(path: &Path) -> Option<StubFacts> {
    let stub = match remote_file::read(path) {
        Ok(stub) => stub,
        Err(error) => {
            warn!(file = %path.display(), %error, "skipping a remote file stub that cannot be read");
            return None;
        }
    };
    let Some(container) = stub.container() else {
        warn!(file = %path.display(), name = %stub.name, "skipping a remote file stub that names no video container");
        return None;
    };
    Some(StubFacts {
        container,
        size: stub.size,
    })
}

pub(super) fn named_as(path: &Path) -> PathBuf {
    if !remote_file::is_remote_file(path) {
        return path.to_path_buf();
    }
    let inner = path.with_extension("");
    if has_video_extension(&inner) {
        inner
    } else {
        path.to_path_buf()
    }
}
