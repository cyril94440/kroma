use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::video::VIDEO_EXTENSIONS;

/// The extension of a stub that stands for a video held by a remote source.
pub const EXTENSION: &str = "kremote";

const MAX_STUB_BYTES: u64 = 64 * 1024;
const MAX_TITLE_BYTES: u64 = 1 << 50;
const MAX_FIELD_CHARS: usize = 1024;

/// A small file standing for a video whose bytes live elsewhere. `source` is the
/// contribution that can locate them, `reference` means something only to that
/// source, `name` is the original file name with its real extension and `size`
/// its real byte size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteFile {
    pub source: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub name: String,
    pub size: u64,
}

impl RemoteFile {
    fn validate(&self) -> Result<(), &'static str> {
        let plain = |text: &str| {
            !text.is_empty() && text.chars().count() <= MAX_FIELD_CHARS && !text.chars().any(char::is_control)
        };
        if self.size > MAX_TITLE_BYTES {
            return Err("a remote file stub declares a size no title has");
        }
        if !plain(&self.source) || !plain(&self.reference) || !plain(&self.name) {
            return Err("a remote file stub holds an empty, oversized or control-character field");
        }
        if self.name.contains(['/', '\\']) || self.name == "." || self.name == ".." {
            return Err("a remote file stub's name is a path");
        }
        Ok(())
    }

    /// The lowercased extension of `name` when it is a video container, else
    /// `None`.
    pub fn container(&self) -> Option<String> {
        let (_, extension) = self.name.rsplit_once('.')?;
        let extension = extension.to_ascii_lowercase();
        VIDEO_EXTENSIONS.contains(&extension.as_str()).then_some(extension)
    }
}

/// Whether `path` names a stub, by extension and ignoring case.
pub fn is_remote_file(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(EXTENSION))
}

/// Reads a stub. A file over 64 KiB is refused before it is parsed, and so is
/// one whose size no real title has or whose name could leave its folder.
pub fn read(path: &Path) -> std::io::Result<RemoteFile> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_STUB_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "a remote file stub is never this large",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_STUB_BYTES + 1).read_to_end(&mut bytes)?;
    let stub: RemoteFile = serde_json::from_slice(&bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    stub.validate()
        .map_err(|why| std::io::Error::new(std::io::ErrorKind::InvalidData, why))?;
    Ok(stub)
}

/// Writes a stub, creating its folder.
pub fn write(path: &Path, file: &RemoteFile) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(file).map_err(std::io::Error::other)?;
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RemoteFile {
        RemoteFile {
            source: "debrid".into(),
            reference: "7:Show/a.mkv".into(),
            name: "Movie.2020.2160p.MKV".into(),
            size: 42,
        }
    }

    #[test]
    fn a_stub_round_trips_through_a_file_with_its_ref_key() {
        let dir = kroma_testing::temp_dir("remote-file");
        let path = dir.path().join("a/b.mkv.kremote");

        write(&path, &sample()).unwrap();

        assert_eq!(read(&path).unwrap(), sample());
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["ref"], "7:Show/a.mkv");
        assert!(raw.get("reference").is_none());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = kroma_testing::temp_dir("remote-file-extra");
        let path = dir.path().join("a.kremote");
        std::fs::write(&path, r#"{"source":"s","ref":"r","name":"n.mkv","size":1,"later":true}"#).unwrap();

        assert_eq!(read(&path).unwrap().reference, "r");
    }

    #[test]
    fn a_file_over_64_kib_is_refused_before_parsing() {
        let dir = kroma_testing::temp_dir("remote-file-big");
        let path = dir.path().join("a.kremote");
        std::fs::write(&path, vec![b' '; 70_000]).unwrap();

        assert_eq!(read(&path).unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn garbage_is_invalid_data() {
        let dir = kroma_testing::temp_dir("remote-file-bad");
        let path = dir.path().join("a.kremote");
        std::fs::write(&path, "not json").unwrap();

        assert_eq!(read(&path).unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn the_extension_is_matched_ignoring_case() {
        assert!(is_remote_file(Path::new("/x/a.mkv.kremote")));
        assert!(is_remote_file(Path::new("A.KREMOTE")));
        assert!(!is_remote_file(Path::new("a.mkv")));
        assert!(!is_remote_file(Path::new("kremote")));
    }

    #[test]
    fn the_container_is_the_real_extension_when_it_is_a_video() {
        assert_eq!(sample().container(), Some("mkv".to_string()));
        let text = RemoteFile { name: "a.srt".into(), ..sample() };
        assert_eq!(text.container(), None);
        let bare = RemoteFile { name: "a".into(), ..sample() };
        assert_eq!(bare.container(), None);
    }

    fn stub_with(body: &str) -> std::io::Result<RemoteFile> {
        let dir = kroma_testing::temp_dir("remote-file-validate");
        let path = dir.path().join("a.kremote");
        std::fs::write(&path, body).unwrap();
        read(&path)
    }

    #[test]
    fn a_size_above_two_to_the_fiftieth_is_refused_so_it_cannot_wrap_into_a_negative_row() {
        let big = format!(r#"{{"source":"s","ref":"r","name":"n.mkv","size":{}}}"#, (1u64 << 50) + 1);
        let max = format!(r#"{{"source":"s","ref":"r","name":"n.mkv","size":{}}}"#, 1u64 << 50);

        assert_eq!(stub_with(&big).unwrap_err().kind(), std::io::ErrorKind::InvalidData);
        assert!(stub_with(&max).is_ok());
        assert!(stub_with(r#"{"source":"s","ref":"r","name":"n.mkv","size":18446744073709551615}"#).is_err());
    }

    #[test]
    fn a_name_that_is_a_path_or_holds_a_control_character_is_refused() {
        for name in ["../a.mkv", "a/b.mkv", "a\\b.mkv", "..", "", "a\nb.mkv", "a\u{0}.mkv"] {
            let body = serde_json::json!({ "source": "s", "ref": "r", "name": name, "size": 1 }).to_string();
            assert!(stub_with(&body).is_err(), "{name:?}");
        }
    }
}
