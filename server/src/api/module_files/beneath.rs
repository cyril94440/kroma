use std::fs::File as StdFile;
use std::path::{Path as FsPath, PathBuf};

pub(super) struct Opened {
    pub file: StdFile,
    pub path: PathBuf,
}

pub(super) fn open_under(root: &FsPath, relative: &FsPath) -> Option<Opened> {
    let file = open_beneath(root, relative)?;
    file.metadata().ok()?.is_file().then(|| Opened {
        file,
        path: root.join(relative),
    })
}

#[cfg(unix)]
fn open_beneath(root: &FsPath, relative: &FsPath) -> Option<StdFile> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    fn open_at(dir: Option<&OwnedFd>, name: &std::ffi::OsStr, flags: libc::c_int) -> Option<OwnedFd> {
        let name = CString::new(name.as_bytes()).ok()?;
        let base = dir.map_or(libc::AT_FDCWD, AsRawFd::as_raw_fd);
        // SAFETY: `name` is a live NUL-terminated string and `base` is a borrowed open descriptor or AT_FDCWD.
        let fd = unsafe { libc::openat(base, name.as_ptr(), flags | libc::O_CLOEXEC | libc::O_NOFOLLOW) };
        // SAFETY: a non-negative `fd` was just returned by `openat` and nothing else owns it.
        (fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(fd) })
    }

    let mut current = open_at(None, root.as_os_str(), libc::O_RDONLY | libc::O_DIRECTORY)?;
    let mut segments = relative.iter().peekable();
    while let Some(segment) = segments.next() {
        let last = segments.peek().is_none();
        let flags = if last { libc::O_RDONLY | libc::O_NONBLOCK } else { libc::O_RDONLY | libc::O_DIRECTORY };
        let next = open_at(Some(&current), segment, flags)?;
        if last {
            return Some(StdFile::from(next));
        }
        current = next;
    }
    None
}

#[cfg(not(unix))]
fn open_beneath(root: &FsPath, relative: &FsPath) -> Option<StdFile> {
    let root = root.canonicalize().ok()?;
    let resolved = root.join(relative).canonicalize().ok()?;
    resolved.starts_with(&root).then(|| StdFile::open(resolved).ok()).flatten()
}

#[cfg(all(test, unix))]
mod tests {
    use std::io::Read;
    use std::os::unix::fs::symlink;

    use super::*;

    fn read(opened: Opened) -> String {
        let mut text = String::new();
        let mut file = opened.file;
        file.read_to_string(&mut text).unwrap();
        text
    }

    fn tree() -> (kroma_testing::TempDir, PathBuf) {
        let dir = kroma_testing::temp_dir("beneath");
        let root = dir.path().join("public");
        std::fs::create_dir_all(root.join("abc")).unwrap();
        std::fs::write(root.join("abc/seg.m4s"), "inside").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "outside").unwrap();
        std::fs::create_dir_all(dir.path().join("elsewhere")).unwrap();
        std::fs::write(dir.path().join("elsewhere/seg.m4s"), "elsewhere").unwrap();
        (dir, root)
    }

    #[test]
    fn a_plain_file_under_the_root_opens() {
        let (_dir, root) = tree();

        let opened = open_under(&root, FsPath::new("abc/seg.m4s")).unwrap();

        assert_eq!(opened.path, root.join("abc/seg.m4s"));
        assert_eq!(read(opened), "inside");
    }

    #[test]
    fn a_symlinked_file_is_refused_even_when_it_points_inside_the_root() {
        let (dir, root) = tree();
        symlink(dir.path().join("secret.txt"), root.join("abc/leak.m4s")).unwrap();
        symlink(root.join("abc/seg.m4s"), root.join("abc/alias.m4s")).unwrap();

        assert!(open_under(&root, FsPath::new("abc/leak.m4s")).is_none());
        assert!(open_under(&root, FsPath::new("abc/alias.m4s")).is_none());
    }

    #[test]
    fn a_folder_swapped_for_a_link_is_never_walked_through() {
        let (dir, root) = tree();
        std::fs::remove_dir_all(root.join("abc")).unwrap();
        symlink(dir.path().join("elsewhere"), root.join("abc")).unwrap();

        assert!(open_under(&root, FsPath::new("abc/seg.m4s")).is_none());
    }

    #[test]
    fn a_root_that_is_itself_a_link_is_refused() {
        let (dir, root) = tree();
        let alias = dir.path().join("alias");
        symlink(&root, &alias).unwrap();

        assert!(open_under(&alias, FsPath::new("abc/seg.m4s")).is_none());
    }

    #[test]
    fn a_folder_or_a_missing_file_is_not_a_file() {
        let (_dir, root) = tree();

        assert!(open_under(&root, FsPath::new("abc")).is_none());
        assert!(open_under(&root, FsPath::new("abc/nope.m4s")).is_none());
        assert!(open_under(&root, FsPath::new("")).is_none());
    }
}
