use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::content::{is_gone, is_held, largest, Content, FilesGone};
use super::naming::{episode_ctx, movie_ctx, resolve_meta, stem_of};
use super::place::{ext_of, Placement};
use super::remote;
use super::replace::drop_replaced;
use super::subtitles::place_with_subtitles;
use crate::peers::downloads::DownloadRow;
use kroma_module_sdk::domain::RequestKind;
use kroma_module_sdk::host::{HostStorage, LibraryFolders};

/// One video of a release in the library: whether this import wrote it, and
/// which episode it is when it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub path: String,
    pub unit: Option<(u32, u32)>,
    pub created: bool,
    pub episodes: u32,
    pub multi_part: bool,
}

/// What importing a row did to the library: its videos, and every file this
/// import wrote (videos and subtitles), which are the only files it may ever
/// take out again.
#[derive(Debug, Default)]
pub struct Landing {
    pub videos: Vec<Placed>,
    pub written: Vec<String>,
}

pub(super) fn place_row<S: HostStorage>(state: &S, row: &DownloadRow) -> Result<Landing> {
    let save_path = row
        .save_path
        .as_deref()
        .ok_or_else(|| anyhow!("download folder unknown (external client did not report it)"))?;
    if !is_held(Path::new(save_path)) {
        bail!(
            "{save_path} is in no folder the download modules may open: add it to Download folders under Admin > Acquisition, or mount its drive if it is listed there already"
        );
    }
    if is_gone(Path::new(save_path)) {
        return Err(FilesGone(save_path.to_string()).into());
    }
    let meta = resolve_meta(state, row)?;

    let content = Content::under(Path::new(save_path), &row.content_rel_paths);
    let videos = content.videos();
    let Some(largest_video) = largest(&videos) else {
        bail!("no video file found under {save_path}");
    };

    let lib = target_library_def(state, meta.kind)?;
    let lib_root = PathBuf::from(
        lib.folders
            .first()
            .ok_or_else(|| anyhow!("library {} has no folder", lib.name))?,
    );
    let tpl = kroma_naming::NamingTemplates::read(|k, d| state.setting_str(k, d));
    let replacing = row.upgrade
        && state.setting_bool("acqReplaceOnUpgrade", true)
        && !crate::rules::store::is_beside(state.store(), &row.id)?;
    let mode = match (row.upgrade, replacing) {
        (true, true) => Placement::Replace,
        (true, false) => Placement::Beside,
        (false, _) => Placement::Skip,
    };
    // A replaced episode is only known once its number is, and what it replaces
    // depends on where the new file landed: collect both as they are placed.
    let mut placed: Vec<(Replaced, String, bool, Claim)> = Vec::new();
    let mut written: Vec<String> = Vec::new();
    match row.kind.as_str() {
        "movie" => {
            let ctx = movie_ctx(&meta, &remote::named(largest_video));
            let dest = lib_root.join(tpl.movie_rel_path(&ctx, ext_of(largest_video)));
            let at = place_with_subtitles(state, largest_video, content.files(), &dest, mode)?;
            record(&mut written, &at);
            placed.push((Replaced::Movie, path_text(&at.video.path), at.video.created, claim_of(largest_video)));
        }
        "episode" => {
            let named = remote::named(largest_video);
            let parsed = kroma_scene::parse_release_name(stem_of(&named));
            let episode = row
                .episodes
                .as_ref()
                .and_then(|e| e.first().copied())
                .or(parsed.episode)
                .ok_or_else(|| anyhow!("could not determine the episode number"))?;
            let season = row.season.or(parsed.season).unwrap_or(1);
            let ctx = episode_ctx(&meta, season, episode, &parsed);
            let dest = lib_root.join(tpl.episode_rel_path(&ctx, ext_of(largest_video)));
            let at = place_with_subtitles(state, largest_video, content.files(), &dest, mode)?;
            record(&mut written, &at);
            placed.push((
                Replaced::Episode(season, episode),
                path_text(&at.video.path),
                at.video.created,
                claim_of(largest_video),
            ));
        }
        "season" => {
            let mut skipped_other_season = 0usize;
            for src in &videos {
                let parsed = kroma_scene::parse_release_name(stem_of(&remote::named(src)));
                let Some(episode) = parsed.episode else {
                    tracing::debug!(file = %src.display(), "season pack: no episode marker, skipped");
                    continue;
                };
                let season = parsed.season.or(row.season).unwrap_or(1);
                // A mixed or complete-series pack carries episodes the grab was
                // not for, and whose upgrade verdict was never computed.
                if row.season.is_some_and(|want| want != season) {
                    skipped_other_season += 1;
                    tracing::debug!(
                        file = %src.display(),
                        season,
                        "season pack: file from another season, skipped"
                    );
                    continue;
                }
                let ctx = episode_ctx(&meta, season, episode, &parsed);
                let dest = lib_root.join(tpl.episode_rel_path(&ctx, ext_of(src)));
                let at = place_with_subtitles(state, src, content.files(), &dest, mode)?;
                record(&mut written, &at);
                placed.push((
                    Replaced::Episode(season, episode),
                    path_text(&at.video.path),
                    at.video.created,
                    claim_of(src),
                ));
            }
            if placed.is_empty() {
                if skipped_other_season > 0 {
                    bail!("season pack held no episode of season {:?}", row.season);
                }
                bail!("season pack had no files with parsable episode numbers");
            }
        }
        other => bail!("unknown download kind {other:?}"),
    }
    if replacing {
        let named: Vec<(Replaced, String)> = placed
            .iter()
            .map(|(unit, path, _, _)| (unit.clone(), path.clone()))
            .collect();
        drop_replaced(state, row, &named, &lib.id);
    }
    let videos = placed
        .into_iter()
        .map(|(unit, path, created, claim)| Placed {
            path,
            created,
            episodes: claim.episodes,
            multi_part: claim.multi_part,
            unit: match unit {
                Replaced::Movie => None,
                Replaced::Episode(season, episode) => Some((season, episode)),
            },
        })
        .collect();
    Ok(Landing { videos, written })
}

struct Claim {
    episodes: u32,
    multi_part: bool,
}

fn claim_of(src: &Path) -> Claim {
    let named = remote::named(src);
    let parsed = kroma_scene::parse_release_name(stem_of(&named));
    let episodes = match (parsed.episode, parsed.episode_end) {
        (Some(first), Some(last)) if last > first => last - first + 1,
        _ => 1,
    };
    Claim {
        episodes,
        multi_part: is_multi_part(stem_of(&named)),
    }
}

fn is_multi_part(stem: &str) -> bool {
    const MARKERS: [&str; 5] = ["cd", "disc", "disk", "part", "pt"];
    let lower = stem.to_ascii_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect();
    let numbered = |rest: &str| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit());
    tokens.iter().enumerate().any(|(at, token)| {
        MARKERS.iter().any(|marker| match token.strip_prefix(marker) {
            Some("") => tokens.get(at + 1).is_some_and(|next| numbered(next)),
            Some(rest) => numbered(rest),
            None => false,
        })
    })
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn record(written: &mut Vec<String>, at: &super::subtitles::WithSubtitles) {
    if at.video.created {
        written.push(path_text(&at.video.path));
    }
    written.extend(at.subtitles.iter().map(|path| path_text(path)));
}

// What an upgrade just improved, resolved to file paths after the new ones land.
#[derive(Clone)]
pub(super) enum Replaced {
    Movie,
    Episode(u32, u32),
}

fn target_library_def<S: HostStorage>(state: &S, kind: RequestKind) -> Result<LibraryFolders> {
    let defs = state.library_folders();
    if defs.is_empty() {
        bail!("no library configured");
    }
    let (setting, wanted_kind) = match kind {
        RequestKind::Movie => ("acqMovieLibrary", "movies"),
        RequestKind::Show => ("acqSeriesLibrary", "shows"),
    };
    let preferred = state.setting_str(setting, "Auto");
    let def: &LibraryFolders = defs
        .iter()
        .find(|d| preferred != "Auto" && !preferred.is_empty() && d.name == preferred)
        .or_else(|| defs.iter().find(|d| d.kind == wanted_kind))
        .or_else(|| defs.first())
        .ok_or_else(|| anyhow!("no library configured"))?;
    Ok(def.clone())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_carrying_a_disc_or_part_marker_is_one_part_of_several() {
        assert!(is_multi_part("Movie.1999.CD1.x264"));
        assert!(is_multi_part("Movie (1999) - part2"));
        assert!(is_multi_part("Movie.1999.Disc 2"));
        assert!(!is_multi_part("Movie.1999.1080p.BluRay.x264-GRP"));
        assert!(!is_multi_part("Partners.1979.720p"));
    }

    #[test]
    fn a_double_episode_file_claims_two_episodes() {
        let double = std::path::PathBuf::from("The.Show.S01E01E02.1080p.mkv");
        let single = std::path::PathBuf::from("The.Show.S01E03.1080p.mkv");

        assert_eq!(claim_of(&double).episodes, 2);
        assert_eq!(claim_of(&single).episodes, 1);
    }
}
