use std::path::Path;

use kroma_primitives::remote_file::{self, RemoteFile};

use super::super::ids::short_hash;
use super::tests::{scan_into, temp_scan_dir};
use crate::model::Kind;

fn stub(path: &Path, name: &str, size: u64) {
    let file = RemoteFile {
        source: "debrid".into(),
        reference: "7:file".into(),
        name: name.into(),
        size,
    };
    remote_file::write(path, &file).unwrap();
}

#[test]
fn a_movie_stub_is_a_title_with_the_real_container_and_the_declared_size() {
    let root_dir = temp_scan_dir();
    let root = root_dir.path();
    let path = root.join("Movie (2020)/Movie (2020).kremote");
    stub(&path, "Movie.2020.2160p.MKV", 42_000_000_000);

    let scan = scan_into("lib1", root);

    let movie = scan.items.values().next().expect("the stub is indexed");
    assert_eq!(movie.kind, Kind::Movie);
    assert_eq!(movie.title, "Movie");
    assert_eq!(movie.year, Some(2020));
    let file = &movie.files[0];
    assert_eq!(file.container, "mkv");
    assert_eq!(file.size, Some(42_000_000_000));
    let abs = root.canonicalize().unwrap().join("Movie (2020)/Movie (2020).kremote");
    assert_eq!(file.abs_path.as_deref(), Some(abs.to_str().unwrap()));
    assert_eq!(file.id, short_hash(abs.to_str().unwrap()));
    assert_eq!(scan.mtimes.len(), 1);
}

#[test]
fn an_episode_stub_groups_under_its_show_like_the_video_it_stands_for() {
    let root_dir = temp_scan_dir();
    let root = root_dir.path();
    stub(&root.join("The Show/Season 01/The Show - S01E02 - Pilot.kremote"), "x.mp4", 7);
    std::fs::write(root.join("The Show/Season 01/The Show - S01E01 - Real.mkv"), b"x").unwrap();

    let scan = scan_into("lib1", root);

    assert_eq!(scan.shows.len(), 1);
    assert_eq!(scan.shows.values().next().unwrap().title, "The Show");
    let episodes: Vec<_> = scan.items.values().filter(|i| i.kind == Kind::Episode).collect();
    assert_eq!(episodes.len(), 2);
    let streamed = episodes.iter().find(|e| e.files[0].container == "mp4").unwrap();
    assert_eq!((streamed.season, streamed.episode), (Some(1), Some(2)));
    assert_eq!(streamed.files[0].size, Some(7));
}

#[test]
fn the_stub_extension_never_leaks_into_a_title() {
    let root_dir = temp_scan_dir();
    let root = root_dir.path();
    stub(&root.join("Heat (1995).mkv.kremote"), "Heat.1995.mkv", 3);
    stub(&root.join("Alien (1979).kremote"), "alien.avi", 3);

    let scan = scan_into("lib1", root);

    let mut titles: Vec<_> = scan.items.values().map(|i| i.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, ["Alien", "Heat"]);
}

#[test]
fn a_stub_that_cannot_be_read_or_names_no_video_is_skipped() {
    let root_dir = temp_scan_dir();
    let root = root_dir.path();
    std::fs::write(root.join("Broken (2001).kremote"), "not json").unwrap();
    stub(&root.join("Subtitle (2002).kremote"), "Subtitle.2002.srt", 3);
    stub(&root.join("Fine (2003).kremote"), "Fine.2003.mkv", 3);

    let scan = scan_into("lib1", root);

    assert_eq!(scan.items.len(), 1);
    assert_eq!(scan.items.values().next().unwrap().title, "Fine");
}
