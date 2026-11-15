use std::path::PathBuf;

use super::frame::HARD_BYTES;
use super::Live;
use crate::clipbuf::CLIPS_DIR;
use crate::publicpath;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    InvalidPath,
    NotFound,
    TooLarge,
}

impl FileError {
    pub fn as_str(self) -> &'static str {
        match self {
            FileError::InvalidPath => "invalidPath",
            FileError::NotFound => "notFound",
            FileError::TooLarge => "tooLarge",
        }
    }
}

impl Live {
    pub fn clip_list(&self) -> Vec<String> {
        let mut paths = Vec::new();
        for folder in std::fs::read_dir(self.clips_path()).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
            for file in std::fs::read_dir(folder.path()).into_iter().flatten().flatten() {
                let path = format!("{CLIPS_DIR}/{}/{}", folder.file_name().to_string_lossy(), file.file_name().to_string_lossy());
                if publicpath::clip_path(&path) {
                    paths.push(path);
                }
            }
        }
        paths.sort();
        paths
    }

    pub fn clip_delete(&self, path: &str) -> Result<bool, FileError> {
        if !publicpath::clip_path(path) {
            return Err(FileError::InvalidPath);
        }
        let file: PathBuf = self.public_dir().join(path);
        let deleted = std::fs::remove_file(&file).is_ok();
        if let Some(folder) = file.parent() {
            let _ = std::fs::remove_dir(folder);
        }
        Ok(deleted)
    }

    pub fn scan_frame(&self, path: &str) -> Result<Vec<u8>, FileError> {
        if !publicpath::scan_frame_path(path) {
            return Err(FileError::InvalidPath);
        }
        let emitted = self.lock().scan.items.iter().any(|item| item.frames.iter().any(|frame| frame == path));
        if !emitted {
            return Err(FileError::NotFound);
        }
        let file = self.public_dir().join(path);
        let size = std::fs::metadata(&file).map_err(|_| FileError::NotFound)?.len();
        if size > HARD_BYTES as u64 {
            return Err(FileError::TooLarge);
        }
        std::fs::read(&file).map_err(|_| FileError::NotFound)
    }
}
