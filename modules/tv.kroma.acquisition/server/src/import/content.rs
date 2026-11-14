//! Which files on disk one download is. An external client saves every torrent
//! into one shared folder, so the engine's own file list decides, and walking
//! the save path is the fallback for a row recorded before engines reported it.

use std::path::{Path, PathBuf};

use kroma_module_sdk::primitives::remote_file::is_remote_file;

use super::remote;

// How far under a download folder a release may bury its files before the
// import stops looking.
const DEEPEST_RELEASE: usize = 6;

pub(super) struct Content {
    files: Vec<PathBuf>,
}

impl Content {
    pub(super) fn under(save_path: &Path, named: &[String]) -> Self {
        let found = resolve(save_path, named);
        let files = if found.is_empty() {
            files_under(save_path)
        } else {
            found
        };
        Self { files }
    }

    pub(super) fn files(&self) -> &[PathBuf] {
        &self.files
    }

    pub(super) fn videos(&self) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|path| {
                (kroma_module_sdk::primitives::video::has_video_extension(path)
                    || is_remote_file(path))
                    && !is_a_sample(&remote::named(path))
            })
            .cloned()
            .collect()
    }
}

#[derive(Debug)]
pub(super) struct FilesGone(pub(super) String);

impl std::fmt::Display for FilesGone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the downloaded files are gone, {} no longer exists",
            self.0
        )
    }
}

impl std::error::Error for FilesGone {}

pub(super) fn is_gone(save_path: &Path) -> bool {
    let absent = matches!(save_path.try_exists(), Ok(false));
    absent
        && save_path
            .parent()
            .is_some_and(|parent| matches!(parent.try_exists(), Ok(true)))
}

pub(super) fn is_held(save_path: &Path) -> bool {
    held_by(save_path, |dir| matches!(dir.try_exists(), Ok(true)))
}

fn held_by(save_path: &Path, opens: impl Fn(&Path) -> bool) -> bool {
    save_path.ancestors().any(opens)
}

pub(super) fn largest(files: &[PathBuf]) -> Option<&Path> {
    files
        .iter()
        .max_by_key(|path| remote::size_of(path))
        .map(PathBuf::as_path)
}

fn resolve(save_path: &Path, named: &[String]) -> Vec<PathBuf> {
    if named.is_empty() {
        return Vec::new();
    }
    let under = files_under(save_path);
    named
        .iter()
        .filter_map(|name| at(save_path, name).or_else(|| by_tail(&under, name)))
        .collect()
}

fn at(save_path: &Path, name: &str) -> Option<PathBuf> {
    let path = save_path.join(name);
    path.is_file().then_some(path)
}

// An engine reports a path relative to its own output folder, which is the save
// path for one and the torrent's folder under it for another.
fn by_tail(under: &[PathBuf], name: &str) -> Option<PathBuf> {
    let tail = Path::new(name);
    under.iter().find(|path| path.ends_with(tail)).cloned()
}

fn files_under(root: &Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .max_depth(DEEPEST_RELEASE)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .collect()
}

fn is_a_sample(path: &Path) -> bool {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains("sample")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn shared_folder(tag: &str) -> kroma_module_sdk::testing::TempDir {
        let scratch = kroma_module_sdk::testing::temp_dir(&format!("content-{tag}"));
        write(
            &scratch.path().join("Other.Film.2160p/Other.Film.2160p.mkv"),
            "xxxxxxxxxxxxxxxxxxxx",
        );
        write(
            &scratch.path().join("Wanted.S01E01/Wanted.S01E01.mkv"),
            "xx",
        );
        scratch
    }

    #[test]
    fn a_torrent_that_names_its_files_imports_none_of_its_neighbours() {
        let shared = shared_folder("named");

        let content = Content::under(
            shared.path(),
            &["Wanted.S01E01/Wanted.S01E01.mkv".to_string()],
        );

        assert_eq!(
            largest(&content.videos()),
            Some(
                shared
                    .path()
                    .join("Wanted.S01E01/Wanted.S01E01.mkv")
                    .as_path()
            )
        );
    }

    #[test]
    fn a_torrent_whose_engine_named_no_file_falls_back_to_the_whole_folder() {
        let shared = shared_folder("unnamed");

        let content = Content::under(shared.path(), &[]);

        assert_eq!(content.files().len(), 2);
        assert_eq!(
            largest(&content.videos()),
            Some(
                shared
                    .path()
                    .join("Other.Film.2160p/Other.Film.2160p.mkv")
                    .as_path()
            )
        );
    }

    #[test]
    fn a_file_the_engine_names_against_another_root_is_found_by_its_tail() {
        let shared = shared_folder("tail");

        let content = Content::under(shared.path(), &["Wanted.S01E01.mkv".to_string()]);

        assert_eq!(content.files().len(), 1);
        assert!(content.files()[0].ends_with("Wanted.S01E01/Wanted.S01E01.mkv"));
    }

    #[test]
    fn a_named_file_that_is_nowhere_is_dropped_rather_than_imported() {
        let shared = shared_folder("missing");

        let content = Content::under(
            shared.path(),
            &[
                "Wanted.S01E01/Wanted.S01E01.mkv".to_string(),
                "Wanted.S01E02/Wanted.S01E02.mkv".to_string(),
            ],
        );

        assert_eq!(content.files().len(), 1);
    }

    #[test]
    fn a_sample_is_never_the_video_a_release_is_imported_from() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-sample");
        write(&scratch.path().join("sample.mkv"), "xxxxxxxxxxxx");
        write(&scratch.path().join("feature.mkv"), "xx");

        let content = Content::under(scratch.path(), &[]);

        assert_eq!(
            largest(&content.videos()),
            Some(scratch.path().join("feature.mkv").as_path())
        );
    }

    #[test]
    fn a_download_folder_removed_from_a_folder_still_there_is_gone() {
        let downloads = kroma_module_sdk::testing::temp_dir("content-gone");

        let gone = is_gone(&downloads.path().join("68165ad60353d090"));

        assert!(gone);
    }

    #[test]
    fn a_download_folder_whose_whole_drive_is_missing_is_not_called_gone() {
        let downloads = kroma_module_sdk::testing::temp_dir("content-offline");

        let gone = is_gone(&downloads.path().join("unmounted/68165ad60353d090"));

        assert!(!gone);
    }

    #[test]
    fn an_empty_download_folder_is_not_gone() {
        let downloads = kroma_module_sdk::testing::temp_dir("content-empty");

        let gone = is_gone(downloads.path());

        assert!(!gone);
    }

    #[test]
    fn a_save_path_under_no_folder_the_module_holds_is_not_held() {
        let opens = |dir: &Path| dir.starts_with("/volume1/video");

        let held = (
            held_by(Path::new("/volume1/downloads/Film.2024"), opens),
            held_by(Path::new("/volume1/video/incoming/Film.2024"), opens),
        );

        assert_eq!(held, (false, true));
    }

    #[test]
    fn the_largest_of_no_video_file_is_nothing() {
        assert!(largest(&[]).is_none());
    }
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    use kroma_module_sdk::primitives::remote_file::{self, RemoteFile};

    fn stub(dir: &Path, rel: &str, name: &str, size: u64) {
        let file = RemoteFile {
            source: "alldebrid".into(),
            reference: format!("7:{name}"),
            name: name.into(),
            size,
        };
        remote_file::write(&dir.join(rel), &file).unwrap();
    }

    #[test]
    fn a_movie_stub_is_the_video_a_release_is_imported_from() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-stub-movie");
        stub(scratch.path(), "Movie/Movie.2020.mkv.kremote", "Movie.2020.mkv", 8_000_000_000);

        let content = Content::under(scratch.path(), &["Movie/Movie.2020.mkv.kremote".to_string()]);

        let videos = content.videos();
        assert_eq!(videos, [scratch.path().join("Movie/Movie.2020.mkv.kremote")]);
        assert_eq!(largest(&videos), Some(videos[0].as_path()));
    }

    #[test]
    fn a_season_pack_of_stubs_yields_every_episode() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-stub-season");
        let names = ["Show.S01E01.mkv", "Show.S01E02.mkv", "Show.S01E03.mkv"];
        for name in names {
            stub(scratch.path(), &format!("Show.S01/{name}.kremote"), name, 1_000);
        }
        let rels: Vec<String> = names.iter().map(|n| format!("Show.S01/{n}.kremote")).collect();

        let content = Content::under(scratch.path(), &rels);

        assert_eq!(content.videos().len(), 3);
    }

    #[test]
    fn a_stub_for_a_sample_is_ignored_like_a_sample() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-stub-sample");
        stub(scratch.path(), "a/x1.kremote", "Movie.sample.mkv", 99_000_000_000);
        stub(scratch.path(), "a/x2.kremote", "Movie.mkv", 5);

        let content = Content::under(scratch.path(), &[]);

        let videos = content.videos();
        assert_eq!(videos, [scratch.path().join("a/x2.kremote")]);
    }

    #[test]
    fn the_declared_size_decides_between_a_stub_and_a_real_file() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-stub-size");
        stub(scratch.path(), "big.mkv.kremote", "big.mkv", 5_000_000_000);
        std::fs::write(scratch.path().join("small.mkv"), vec![0u8; 100_000]).unwrap();
        let content = Content::under(scratch.path(), &[]);

        let videos = content.videos();

        assert_eq!(largest(&videos), Some(scratch.path().join("big.mkv.kremote").as_path()));
    }

    #[test]
    fn a_stub_is_not_mistaken_for_a_video_when_it_is_a_plain_file_without_the_extension() {
        let scratch = kroma_module_sdk::testing::temp_dir("content-stub-plain");
        std::fs::write(scratch.path().join("readme.txt"), "x").unwrap();

        assert!(Content::under(scratch.path(), &[]).videos().is_empty());
    }
}
