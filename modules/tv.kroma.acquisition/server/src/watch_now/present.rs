use anyhow::Result;
use kroma_module_sdk::db;
use kroma_module_sdk::domain::RequestKind;
use kroma_module_sdk::host::HostStorage;

use crate::auto::title_kind;
use crate::failures::{read_downloads, Download};

/// Whether any library already holds the title, or the asked episode of it,
/// whoever may see that library.
pub fn in_library<S: HostStorage>(
    state: &S,
    kind: RequestKind,
    tmdb_id: u64,
    unit: Option<(u32, u32)>,
) -> Result<bool> {
    let conn = state.db().get()?;
    Ok(match (kind, unit) {
        (RequestKind::Movie, _) => db::movie_item_by_tmdb(&conn, tmdb_id)?.is_some(),
        (RequestKind::Show, None) => db::show_by_tmdb(&conn, tmdb_id)?.is_some(),
        (RequestKind::Show, Some(unit)) => match db::show_by_tmdb(&conn, tmdb_id)? {
            Some(show) => db::episodes_present(&conn, &show)?.contains(&unit),
            None => false,
        },
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum OnItsWay {
    Nothing,
    Underway,
    Ready(Box<str>),
}

/// What a live download already does for the asked scope: still fetching, or
/// finished and waiting to be imported, in which case that one is used rather
/// than a second grab.
pub fn on_its_way<S: HostStorage>(
    state: &S,
    kind: RequestKind,
    tmdb_id: u64,
    unit: Option<(u32, u32)>,
) -> Result<OnItsWay> {
    let rows = read_downloads(state)?;
    let covering: Vec<&Download> = rows
        .iter()
        .filter(|row| covers(row, kind, tmdb_id, unit))
        .collect();
    if let Some(ready) = covering.iter().find(|row| row.status == "completed") {
        return Ok(OnItsWay::Ready(ready.id.clone().into()));
    }
    Ok(if covering.iter().any(|row| row.unfinished()) {
        OnItsWay::Underway
    } else {
        OnItsWay::Nothing
    })
}

fn covers(row: &Download, kind: RequestKind, tmdb_id: u64, unit: Option<(u32, u32)>) -> bool {
    if row.tmdb_id != tmdb_id || title_kind(&row.kind) != kind {
        return false;
    }
    match unit {
        None => true,
        Some((season, episode)) => {
            row.season == Some(season)
                && row
                    .episodes
                    .as_ref()
                    .is_none_or(|episodes| episodes.contains(&episode))
        }
    }
}
