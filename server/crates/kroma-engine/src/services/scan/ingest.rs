//! Cataloguing a handful of files the moment they land, without walking a
//! library: the same indexing a scan does, written without the pruning a full
//! scan does.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::model::{Library, LibraryKind, MediaItem, Show};
use crate::services::settings::LibraryDef;

use super::walk::{file_meta, index_file, Accumulator, Folder};

/// A file now in the catalogue, and the item it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingested {
    pub file_id: String,
    pub item_id: String,
    pub abs_path: String,
}

/// Adds these files to the catalogue. Each must lie under a configured library
/// folder, because its title is read off the path beneath that folder.
pub fn ingest_files(state: &crate::state::SharedState, paths: &[PathBuf]) -> Result<Vec<Ingested>> {
    let defs = crate::services::settings::library_defs(&state.settings, &state.config);
    let (items, shows, mtimes) = index_under(&defs, paths)?;
    let found = ingested(&items);
    let libraries = libraries_of(&defs, &items);
    crate::db::sync_added(&state.db, &libraries, &shows, &items, &mtimes)?;
    state
        .events
        .publish(crate::infra::events::ServerEvent::LibraryUpdated);
    Ok(found)
}

/// Takes files that left the disk out of the catalogue.
pub fn forget_files(state: &crate::state::SharedState, paths: &[String]) -> Result<()> {
    crate::db::forget_files(&state.db, paths)?;
    state
        .events
        .publish(crate::infra::events::ServerEvent::LibraryUpdated);
    Ok(())
}

type Indexed = (Vec<MediaItem>, Vec<Show>, HashMap<String, Option<i64>>);

fn index_under(defs: &[LibraryDef], paths: &[PathBuf]) -> Result<Indexed> {
    let mut items: HashMap<String, MediaItem> = HashMap::new();
    let mut shows: HashMap<String, Show> = HashMap::new();
    let mut mtimes = HashMap::new();
    let mut seen_ids = HashSet::new();
    let (mut movie_seen, mut episode_seen) = (false, false);
    for path in paths {
        let Some((def, root)) = library_of(defs, path) else {
            bail!("{} is under no library folder", path.display());
        };
        let folder = Folder::of(&def.id, &root);
        let mut acc = Accumulator {
            items: &mut items,
            shows: &mut shows,
            mtimes: &mut mtimes,
            lib_item_ids: &mut seen_ids,
            movie_seen: &mut movie_seen,
            episode_seen: &mut episode_seen,
        };
        index_file(&folder, path, file_meta(path), &mut acc);
    }
    Ok((
        items.into_values().collect(),
        shows.into_values().collect(),
        mtimes,
    ))
}

fn library_of<'a>(defs: &'a [LibraryDef], path: &Path) -> Option<(&'a LibraryDef, PathBuf)> {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    defs.iter().find_map(|def| {
        def.folders.iter().find_map(|folder| {
            let root = PathBuf::from(folder);
            let abs = root.canonicalize().unwrap_or_else(|_| root.clone());
            resolved.starts_with(&abs).then_some((def, root))
        })
    })
}

fn libraries_of(defs: &[LibraryDef], items: &[MediaItem]) -> Vec<Library> {
    let held: HashSet<&str> = items.iter().map(|item| item.library.as_str()).collect();
    defs.iter()
        .filter(|def| held.contains(def.id.as_str()))
        .map(|def| Library {
            id: def.id.clone(),
            name: def.name.clone(),
            kind: match def.kind.as_str() {
                "shows" => LibraryKind::Shows,
                "mixed" => LibraryKind::Mixed,
                _ => LibraryKind::Movies,
            },
            path: def.folders.join(", "),
            item_count: 0,
        })
        .collect()
}

fn ingested(items: &[MediaItem]) -> Vec<Ingested> {
    items
        .iter()
        .flat_map(|item| {
            item.files.iter().filter_map(|file| {
                Some(Ingested {
                    file_id: file.id.clone(),
                    item_id: item.id.clone(),
                    abs_path: file.abs_path.clone()?,
                })
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
