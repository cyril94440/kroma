use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn dest(folder: &Path, rel: &str) -> PathBuf {
    rel.split('/')
        .fold(folder.to_path_buf(), |path, part| path.join(part))
}

pub fn part_of(dest: &Path) -> PathBuf {
    suffixed(dest, ".part")
}

pub fn chunk_of(dest: &Path) -> PathBuf {
    suffixed(dest, ".chunk")
}

fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

pub fn len(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|meta| meta.len())
}

pub fn is_complete(dest: &Path, size: u64) -> bool {
    len(dest).is_some_and(|have| size == 0 || have == size)
}

pub fn mark_used(path: &Path, at: SystemTime) {
    let opened = std::fs::OpenOptions::new().write(true).open(path);
    if let Ok(file) = opened {
        let _ = file.set_modified(at);
    }
}
