//! The subtitle files a release ships beside its video, imported with it.

use std::path::{Path, PathBuf};

use anyhow::Result;

use kroma_module_sdk::host::HostCtx;
use kroma_module_sdk::primitives::external_subtitles::beside;

use super::place::{place, Landed, Placement};
use super::remote::named;

/// Place `src` in the library, then every subtitle file beside it, renamed to
/// the stem the video landed under so the catalogue matches the two again.
/// Answers where the video landed, whether this call wrote it, and the subtitle
/// files it wrote.
pub(super) fn place_with_subtitles(
    host: &dyn HostCtx,
    src: &Path,
    around: &[PathBuf],
    dest: &Path,
    mode: Placement,
) -> Result<WithSubtitles> {
    let video = place(host, src, dest, mode)?;
    let subtitles = subtitles_beside(host, src, around, &video.path, mode);
    if !subtitles.is_empty() {
        tracing::info!(video = %video.path.display(), placed = subtitles.len(), "subtitle files imported beside a video");
    }
    Ok(WithSubtitles { video, subtitles })
}

pub(super) struct WithSubtitles {
    pub video: Landed,
    pub subtitles: Vec<PathBuf>,
}

fn subtitles_beside(
    host: &dyn HostCtx,
    src: &Path,
    around: &[PathBuf],
    at: &Path,
    mode: Placement,
) -> Vec<PathBuf> {
    let Some(stem) = at.file_stem().and_then(std::ffi::OsStr::to_str) else {
        return Vec::new();
    };
    let folder = at.parent().unwrap_or_else(|| Path::new("."));
    let around: Vec<PathBuf> = around.iter().map(|path| named(path)).collect();
    beside(&named(src), &around)
        .iter()
        .filter_map(|found| {
            let to = folder.join(found.named_after(stem));
            match place(host, &found.path, &to, mode) {
                Ok(landed) => landed.created.then_some(landed.path),
                Err(e) => {
                    tracing::warn!(file = %found.path.display(), error = %format!("{e:#}"), "a subtitle file would not import");
                    None
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kroma_module_sdk::host::testing::StubHost;

    fn scratch(name: &str) -> kroma_module_sdk::testing::TempDir {
        kroma_module_sdk::testing::temp_dir(&format!("import-subs-{name}"))
    }

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    struct Release {
        _scratch: kroma_module_sdk::testing::TempDir,
        video: PathBuf,
        around: Vec<PathBuf>,
        dest: PathBuf,
    }

    fn release(tag: &str, names: &[&str]) -> Release {
        let scratch = scratch(tag);
        let folder = scratch.path().join("downloads/Old.Release.1080p");
        let video = folder.join("Old.Release.1080p.mkv");
        write(&video, "bytes");
        let mut around = vec![video.clone()];
        for name in names {
            let file = folder.join(name);
            write(&file, "1\n00:00:01,000 --> 00:00:02,000\nhello\n");
            around.push(file);
        }
        let dest = scratch
            .path()
            .join("library/The Matrix (1999)/The Matrix (1999).mkv");
        Release {
            _scratch: scratch,
            video,
            around,
            dest,
        }
    }

    #[test]
    fn a_subtitle_file_follows_the_video_under_its_new_name() {
        let r = release("follows", &["Old.Release.1080p.fr.srt"]);

        let at = place_with_subtitles(&StubHost::new(), &r.video, &r.around, &r.dest, Placement::Skip).unwrap().video.path;

        assert_eq!(at, r.dest);
        assert!(
            r.dest.with_file_name("The Matrix (1999).fr.srt").exists(),
            "the subtitle did not land beside the video"
        );
    }

    #[test]
    fn a_subs_folder_is_flattened_beside_the_video() {
        let r = release("flattened", &["Subs/2_English.srt", "Subs/3_French.srt"]);

        place_with_subtitles(&StubHost::new(), &r.video, &r.around, &r.dest, Placement::Skip).unwrap();

        let folder = r.dest.parent().unwrap();
        assert!(folder.join("The Matrix (1999).en.srt").exists());
        assert!(folder.join("The Matrix (1999).fr.srt").exists());
    }

    #[test]
    fn a_picture_subtitle_is_left_where_it_was() {
        let r = release("picture", &["Old.Release.1080p.fr.sub"]);

        place_with_subtitles(&StubHost::new(), &r.video, &r.around, &r.dest, Placement::Skip).unwrap();

        let folder = r.dest.parent().unwrap();
        assert_eq!(std::fs::read_dir(folder).unwrap().count(), 1);
    }

    #[test]
    fn a_release_that_ships_no_subtitle_file_still_imports() {
        let r = release("none", &[]);

        let at = place_with_subtitles(&StubHost::new(), &r.video, &r.around, &r.dest, Placement::Skip).unwrap().video.path;

        assert!(at.exists());
    }
}
