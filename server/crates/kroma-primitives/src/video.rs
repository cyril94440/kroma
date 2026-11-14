//! Which files hold a picture a player can open.

use std::path::Path;

pub const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "mov", "webm", "avi", "ts"];

pub fn has_video_extension(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|e| VIDEO_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether `path` is a file the library indexes as a title: a video container,
/// or a [`crate::remote_file`] stub standing for one.
pub fn is_media_file(path: &Path) -> bool {
    has_video_extension(path) || crate::remote_file::is_remote_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_is_recognised_however_its_extension_is_cased() {
        let held = ["/movies/Film.mkv", "/movies/Film.MP4", "clip.MOV", "a.ts"];

        assert!(held.iter().all(|p| has_video_extension(Path::new(p))));
    }

    #[test]
    fn a_file_beside_a_film_is_not_the_film() {
        let beside = ["/movies/poster.jpg", "/movies/subs.srt", "/movies/noext"];

        assert!(!beside.iter().any(|p| has_video_extension(Path::new(p))));
    }

    #[test]
    fn a_stub_is_a_title_though_it_is_not_a_video_container() {
        assert!(is_media_file(Path::new("/movies/Film (2020).kremote")));
        assert!(is_media_file(Path::new("/movies/Film.MKV")));
        assert!(!has_video_extension(Path::new("/movies/Film (2020).kremote")));
        assert!(!is_media_file(Path::new("/movies/Film.srt")));
    }
}
