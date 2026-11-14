//! The phase-1 filesystem walk: a parallel jwalk traversal of one library
//! folder that `stat`s each video file and groups it (via [`crate::domain::naming`])
//! into the shared logical-item / show maps.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use jwalk::{Parallelism, WalkDirGeneric};
use tracing::{debug, warn};

use kroma_primitives::remote_file;

use crate::domain::naming;
use crate::model::{MediaFile, MediaItem, Show};

mod stub;

use stub::{named_as, readable_stub};

use super::ids::{detect_edition, short_hash};
use super::index::index_parsed;
use super::now_iso8601;

pub use kroma_media::video::{has_video_extension, is_media_file, VIDEO_EXTENSIONS};

/// Scan one folder belonging to `lib_id`, accumulating items (by logical id) and
/// shows into the shared maps. Flags/ids track what this library contributed so
/// the caller can compute its kind + item count across all its folders.
///
/// Returns how many entries the walk could not read: a symlink whose target is
/// not mounted resolves to nothing, and a whole library of those otherwise
/// scans clean and empty.
#[allow(clippy::too_many_arguments)]
pub(super) fn scan_root(
    lib_id: &str,
    root: &Path,
    items: &mut HashMap<String, MediaItem>,
    shows: &mut HashMap<String, Show>,
    mtimes: &mut HashMap<String, Option<i64>>,
    lib_item_ids: &mut std::collections::HashSet<String>,
    movie_seen: &mut bool,
    episode_seen: &mut bool,
) -> usize {
    let folder = Folder::of(lib_id, root);
    let abs_root = folder.abs_root.clone();
    let mut acc = Accumulator {
        items,
        shows,
        mtimes,
        lib_item_ids,
        movie_seen,
        episode_seen,
    };

    // Phase-1 walk: jwalk traverses directories on a large thread pool and stats
    // files *in that pool*, so the readdir/stat network round-trips over SMB run
    // concurrently instead of serially (minutes → seconds). Synology `@eaDir`
    // and hidden dirs are pruned from the descent.
    let descended = Arc::new(Mutex::new(HashSet::from([abs_root.clone()])));
    let walk = WalkDirGeneric::<((), FileMeta)>::new(root)
        .follow_links(true)
        .skip_hidden(false)
        .parallelism(Parallelism::RayonNewPool(walk_threads()))
        .process_read_dir(move |_depth, _path, _state, children| {
            prepare_children(&descended, children);
        });

    let mut unreadable = 0usize;
    let mut first_unreadable: Option<PathBuf> = None;
    for result in walk {
        let entry = match result {
            Ok(entry) => entry,
            Err(err) => {
                unreadable += 1;
                first_unreadable.get_or_insert_with(|| err.path().unwrap_or(root).to_path_buf());
                debug!("skipping an unreadable entry: {err}");
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if !is_media_file(&path) {
            continue;
        }
        let meta = entry.client_state;
        index_file(&folder, &path, meta, &mut acc);

    }

    if let Some(path) = first_unreadable {
        warn!(
            folder = %root.display(),
            unreadable,
            example = %path.display(),
            "entries skipped: a symlink whose target is not mounted reads as missing"
        );
    }
    unreadable
}

pub(super) struct Folder<'a> {
    lib_id: &'a str,
    root: &'a Path,
    abs_root: PathBuf,
    added_at: String,
}

impl<'a> Folder<'a> {
    // Resolve the root once (cheap, single syscall) so file abs-paths are stable
    // without a per-file `canonicalize()` (which is very slow over SMB).
    pub(super) fn of(lib_id: &'a str, root: &'a Path) -> Self {
        Self {
            lib_id,
            root,
            abs_root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
            added_at: now_iso8601(),
        }
    }
}

pub(super) struct Accumulator<'a> {
    pub items: &'a mut HashMap<String, MediaItem>,
    pub shows: &'a mut HashMap<String, Show>,
    pub mtimes: &'a mut HashMap<String, Option<i64>>,
    pub lib_item_ids: &'a mut HashSet<String>,
    pub movie_seen: &'a mut bool,
    pub episode_seen: &'a mut bool,
}

/// Folds one media file under `folder` into what a scan is collecting. `meta` is
/// its size and mtime, when they could be read.
pub(super) fn index_file(
    folder: &Folder,
    path: &Path,
    meta: FileMeta,
    acc: &mut Accumulator,
) {
    let rel = path
        .strip_prefix(folder.root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let abs = folder.abs_root.join(&rel);
    let (mut size, mtime) = match meta {
        Some((s, m)) => (Some(s), Some(m)),
        None => (None, None),
    };
    let mut container = path
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if remote_file::is_remote_file(path) {
        let Some(stub) = readable_stub(path) else {
            return;
        };
        container = stub.container.clone();
        size = Some(stub.size);
    }
    let named = named_as(path);
    let file_name = named
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("");
    let edition = detect_edition(file_name);
    let file = MediaFile {
        remote: remote_file::is_remote_file(path),
        id: short_hash(&abs.to_string_lossy()),
        rel_path: Some(rel),
        container,
        duration_ms: None,
        video: None,
        audio: None,
        audio_tracks: Vec::new(),
        subtitles: Vec::new(),
        size,
        edition,
        probed: false,
        unreadable: None,
        edition_id: None,
        abs_path: Some(abs.to_string_lossy().to_string()),
    };
    index_parsed(
        naming::parse(folder.root, &named),
        folder.lib_id,
        &folder.added_at,
        file,
        mtime,
        acc.items,
        acc.shows,
        acc.mtimes,
        acc.lib_item_ids,
        acc.movie_seen,
        acc.episode_seen,
    );
}

fn prepare_children(
    descended: &Mutex<HashSet<PathBuf>>,
    children: &mut Vec<jwalk::Result<jwalk::DirEntry<((), FileMeta)>>>,
) {
    children.retain(|res| match res {
        Ok(e) => !(e.file_type().is_dir() && is_pruned_dir(&e.file_name)),
        Err(_) => true,
    });
    for e in children.iter_mut().flatten() {
        if e.file_type().is_file() {
            e.client_state = file_meta(&e.path());
        } else if e.file_type().is_dir()
            && e.path_is_symlink()
            && !first_descent(descended, &e.path())
        {
            e.read_children = None;
        }
    }
}

// jwalk's own guard compares a symlink's raw target against the ancestor paths,
// so `link -> /abs/ancestor` is caught and `link -> .` is not: the kernel's
// 32-symlink ceiling then bounds the depth, which two such links in one
// directory turn into 2^32 paths.
fn first_descent(descended: &Mutex<HashSet<PathBuf>>, dir: &Path) -> bool {
    let Ok(resolved) = dir.canonicalize() else {
        return false;
    };
    match descended.lock() {
        Ok(mut seen) => seen.insert(resolved),
        Err(poisoned) => poisoned.into_inner().insert(resolved),
    }
}

// Per-file metadata carried through the parallel jwalk: (size, mtime-secs).
pub(super) type FileMeta = Option<(u64, i64)>;

// Concurrency for the directory walk. Metadata ops are latency-bound (not CPU),
// so more threads than cores overlap the round-trips but a NAS serving its
// own local disks gains nothing past a handful, and 64 idle-blocked threads
// still cost stacks + scheduler churn on a 2-4 core box. `KROMA_WALK_THREADS`
// overrides for genuinely remote mounts.
fn walk_threads() -> usize {
    if let Some(n) = std::env::var("KROMA_WALK_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return n;
    }
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4);
    (cores * 4).clamp(8, 32)
}

// Directories pruned from the walk: Synology metadata (`@eaDir`) and hidden.
fn is_pruned_dir(name: &std::ffi::OsStr) -> bool {
    let n = name.to_string_lossy();
    n == "@eaDir" || n.starts_with('.')
}

// `stat` one file (run inside the parallel walk pool): (size, mtime-as-unix-secs).
pub(super) fn file_meta(path: &Path) -> FileMeta {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Some((md.len(), mtime))
}

#[cfg(test)]
mod stub_tests;
#[cfg(test)]
mod tests;
