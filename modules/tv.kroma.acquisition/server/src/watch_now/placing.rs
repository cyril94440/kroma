use anyhow::Result;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::primitives::now_ms;

use super::provisional::{self, Provisional};
use super::{reject, Imported, Release, Reply};
use crate::import::{Landing, Placed};
use crate::peers::downloads::DownloadRow;

pub(super) enum Placing {
    Reply(Reply),
    Rejected,
}

pub(super) fn place_ready<S: HostStorage>(
    state: &S,
    request_id: &str,
    download_id: &str,
    unit: Option<(u32, u32)>,
    key: &str,
) -> Result<Option<Reply>> {
    let Some(row) = crate::peers::downloads::completed(state)?
        .into_iter()
        .find(|row| row.id == download_id)
    else {
        return Ok(None);
    };
    Ok(match place(state, request_id, row, key, unit, Vec::new())? {
        Placing::Reply(reply) => Some(reply),
        Placing::Rejected => None,
    })
}

pub(super) fn place<S: HostStorage>(
    state: &S,
    request_id: &str,
    download: DownloadRow,
    key: &str,
    unit: Option<(u32, u32)>,
    wanted_ids: Vec<String>,
) -> Result<Placing> {
    let Some((row, landing)) = crate::import::place_download(state, &download.id)? else {
        return already_taken(state, request_id, &download, key, unit);
    };
    let held = Provisional {
        download_id: row.id.clone(),
        request_id: Some(request_id.to_string()),
        kind: crate::auto::title_kind(&row.kind).as_str().into(),
        tmdb_id: row.tmdb_id,
        season: row.season,
        release_title: row.release_title.clone(),
        paths: landing.videos.iter().map(|p| p.path.clone()).collect(),
        written: landing.written.clone(),
        wanted_ids,
        created_at: now_ms(),
    };
    provisional::save(state.store(), &held)?;
    let Some(primary) = primary_of(&landing.videos, unit) else {
        let reason = "the pack does not hold the episode asked for";
        reject::blocklist_release(state, &held, reason)?;
        reject::undo(state, &held, reason)?;
        return Ok(Placing::Rejected);
    };
    if !primary.created {
        reject::undo(state, &held, "already in the library")?;
        return Ok(Placing::Reply(Reply::Present {
            request_id: Some(request_id.to_string()),
        }));
    }
    Ok(Placing::Reply(Reply::Imported(Imported {
        request_id: request_id.to_string(),
        download_id: row.id,
        release: release_of(key, &row.release_title),
        paths: created_paths(&landing),
        primary_path: primary.path.clone(),
        episodes: primary.episodes,
        multi_part: primary.multi_part,
    })))
}

fn already_taken<S: HostStorage>(
    state: &S,
    request_id: &str,
    download: &DownloadRow,
    key: &str,
    unit: Option<(u32, u32)>,
) -> Result<Placing> {
    let Some(paths) = crate::import::recorded_paths(state, &download.id)? else {
        return Ok(Placing::Reply(Reply::Requested {
            request_id: request_id.to_string(),
        }));
    };
    let primary = paths
        .iter()
        .find(|path| unit.is_none() || unit_of_name(path) == unit)
        .or_else(|| paths.first());
    Ok(match primary {
        Some(primary) => Placing::Reply(Reply::Imported(Imported {
            request_id: request_id.to_string(),
            download_id: download.id.clone(),
            release: release_of(key, &download.release_title),
            primary_path: primary.clone(),
            paths,
            episodes: 1,
            multi_part: false,
        })),
        None => Placing::Reply(Reply::Requested {
            request_id: request_id.to_string(),
        }),
    })
}

fn release_of(key: &str, title: &str) -> Release {
    Release {
        key: key.to_string(),
        title: title.to_string(),
        claimed_height: kroma_scene::parse_release_name(title)
            .resolution
            .map(height_of),
    }
}

fn unit_of_name(path: &str) -> Option<(u32, u32)> {
    let stem = std::path::Path::new(path).file_stem()?.to_str()?;
    let parsed = kroma_scene::parse_release_name(stem);
    Some((parsed.season?, parsed.episode?))
}

fn created_paths(landing: &Landing) -> Vec<String> {
    landing
        .videos
        .iter()
        .filter(|p| p.created)
        .map(|p| p.path.clone())
        .collect()
}

pub(super) fn primary_of(placed: &[Placed], unit: Option<(u32, u32)>) -> Option<&Placed> {
    match unit {
        None => placed.first(),
        Some(wanted) => placed.iter().find(|p| p.unit == Some(wanted)),
    }
}

fn height_of(resolution: kroma_scene::Res) -> u32 {
    match resolution {
        kroma_scene::Res::Sd => 480,
        kroma_scene::Res::R720 => 720,
        kroma_scene::Res::R1080 => 1080,
        kroma_scene::Res::R2160 => 2160,
    }
}
