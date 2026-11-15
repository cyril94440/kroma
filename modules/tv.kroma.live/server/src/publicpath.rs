use crate::clipbuf::{valid_name, CLIPS_DIR};

const SCAN_PREFIX: &str = "scan-";
const MAX_SEGMENT: usize = 96;

fn token(text: &str) -> bool {
    (16..=128).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn file_stem(text: &str) -> bool {
    (1..=MAX_SEGMENT).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn clip_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let [dir, folder, file] = parts[..] else { return false };
    dir == CLIPS_DIR && token(folder) && file.strip_suffix(".mp4").is_some_and(valid_name)
}

pub fn scan_frame_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let [folder, file] = parts[..] else { return false };
    folder.strip_prefix(SCAN_PREFIX).is_some_and(token) && file.strip_suffix(".jpg").is_some_and(file_stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_clip_path_is_the_three_part_path_clip_cut_returns() {
        assert!(clip_path(&format!("clips/{TOKEN}/espn-1-but-2.mp4")));
        for bad in [
            "clips/../x/but.mp4".to_string(),
            format!("clips/{TOKEN}/../but.mp4"),
            format!("clips/{TOKEN}/Bad Name.mp4"),
            format!("clips/{TOKEN}/but.mkv"),
            format!("/clips/{TOKEN}/but.mp4"),
            format!("clips/{TOKEN}/sub/but.mp4"),
            format!("buffer/{TOKEN}/but.mp4"),
            "clips/zz/but.mp4".to_string(),
            String::new(),
        ] {
            assert!(!clip_path(&bad), "{bad}");
        }
    }

    #[test]
    fn a_scan_frame_path_is_a_scan_folder_and_a_jpeg_name_and_nothing_else() {
        assert!(scan_frame_path(&format!("scan-{TOKEN}/101.jpg")));
        assert!(scan_frame_path(&format!("scan-{TOKEN}/101-first.jpg")));
        for bad in [
            format!("scan-{TOKEN}/../x.jpg"),
            format!("scan-{TOKEN}/..%2fx.jpg"),
            format!("scan-{TOKEN}/101.png"),
            format!("scan-{TOKEN}/.jpg"),
            format!("scan-{TOKEN}/a/101.jpg"),
            format!("scan-{TOKEN}\\101.jpg"),
            format!("clips/{TOKEN}/101.jpg"),
            "scan-xyz/101.jpg".to_string(),
            "scan-/101.jpg".to_string(),
            "101.jpg".to_string(),
        ] {
            assert!(!scan_frame_path(&bad), "{bad}");
        }
    }
}
