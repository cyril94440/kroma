//! A `.kremote` stub stands for a video held by a remote source: it is imported
//! like the video, under the name and with the size the stub declares.

use std::path::{Path, PathBuf};

use kroma_module_sdk::primitives::remote_file::{self, RemoteFile};

pub(super) fn stub(path: &Path) -> Option<RemoteFile> {
    remote_file::is_remote_file(path)
        .then(|| remote_file::read(path).ok())
        .flatten()
}

pub(super) fn named(path: &Path) -> PathBuf {
    let real = stub(path)
        .and_then(|file| Path::new(&file.name).file_name().map(PathBuf::from));
    match real {
        Some(name) => path.with_file_name(name),
        None => path.to_path_buf(),
    }
}

pub(super) fn size_of(path: &Path) -> u64 {
    match stub(path) {
        Some(file) => file.size,
        None => std::fs::metadata(path).map_or(0, |meta| meta.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub_at(dir: &Path, rel: &str, name: &str, size: u64) -> PathBuf {
        let path = dir.join(rel);
        let file = RemoteFile {
            source: "alldebrid".into(),
            reference: "7:x".into(),
            name: name.into(),
            size,
        };
        remote_file::write(&path, &file).unwrap();
        path
    }

    #[test]
    fn a_stub_is_named_and_sized_as_the_video_it_stands_for() {
        let dir = kroma_module_sdk::testing::temp_dir("import-remote");
        let path = stub_at(dir.path(), "a/Movie.2020.2160p.mkv.kremote", "Movie.2020.2160p.mkv", 9_000_000_000);

        assert_eq!(named(&path), dir.path().join("a/Movie.2020.2160p.mkv"));
        assert_eq!(size_of(&path), 9_000_000_000);
    }

    #[test]
    fn a_real_file_is_itself() {
        let dir = kroma_module_sdk::testing::temp_dir("import-remote-real");
        let path = dir.path().join("a.mkv");
        std::fs::write(&path, "xxxx").unwrap();

        assert_eq!(named(&path), path);
        assert_eq!(size_of(&path), 4);
    }

    #[test]
    fn a_stub_that_cannot_be_read_is_just_a_small_file() {
        let dir = kroma_module_sdk::testing::temp_dir("import-remote-bad");
        let path = dir.path().join("a.mkv.kremote");
        std::fs::write(&path, "nope").unwrap();

        assert_eq!(named(&path), path);
        assert_eq!(size_of(&path), 4);
    }

    #[test]
    fn a_hostile_name_cannot_steer_the_virtual_path_out_of_its_folder() {
        let dir = kroma_module_sdk::testing::temp_dir("import-remote-hostile");
        let path = stub_at(dir.path(), "a/x.kremote", "../../etc/x.mkv", 1);

        assert_eq!(named(&path), dir.path().join("a/x.mkv"));
    }
}
