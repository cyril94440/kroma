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
