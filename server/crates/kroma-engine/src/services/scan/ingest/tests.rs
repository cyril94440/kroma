use super::*;
use kroma_primitives::remote_file::{self, RemoteFile};

fn def(root: &Path) -> LibraryDef {
    LibraryDef {
        id: "lib".into(),
        name: "Films".into(),
        kind: "movies".into(),
        folders: vec![root.to_string_lossy().into_owned()],
        auto_scan: true,
    }
}

fn stub(path: &Path) {
    let file = RemoteFile {
        source: "alldebrid".into(),
        reference: "7:a.mkv".into(),
        name: "a.mkv".into(),
        size: 5,
    };
    remote_file::write(path, &file).unwrap();
}

#[test]
fn a_file_is_indexed_under_the_library_folder_that_holds_it() {
    let root = kroma_testing::temp_dir("ingest-under");
    let file = root.path().join("Dune (2021)/Dune (2021).mkv.kremote");
    stub(&file);

    let (items, _, mtimes) = index_under(&[def(root.path())], std::slice::from_ref(&file)).unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].title, "Dune");
    assert_eq!(items[0].files.len(), 1);
    assert_eq!(mtimes.len(), 1);
    let found = ingested(&items);
    assert_eq!(found[0].item_id, items[0].id);
    assert!(found[0].abs_path.ends_with("Dune (2021).mkv.kremote"));
}

#[test]
fn a_season_pack_lands_as_one_show_with_an_item_per_episode() {
    let root = kroma_testing::temp_dir("ingest-pack");
    let mut paths = Vec::new();
    for episode in 1..=3 {
        let path = root.path().join(format!(
            "The Show/Season 1/The Show - S01E0{episode}.mkv.kremote"
        ));
        stub(&path);
        paths.push(path);
    }

    let (items, shows, _) = index_under(&[def(root.path())], &paths).unwrap();

    assert_eq!(shows.len(), 1);
    assert_eq!(items.len(), 3);
}

#[test]
fn a_file_outside_every_library_folder_is_refused() {
    let root = kroma_testing::temp_dir("ingest-outside");
    let elsewhere = kroma_testing::temp_dir("ingest-elsewhere");
    let file = elsewhere.path().join("a.mkv.kremote");
    stub(&file);

    let refused = index_under(&[def(root.path())], &[file])
        .unwrap_err()
        .to_string();

    assert!(refused.contains("under no library folder"), "{refused}");
}

#[test]
fn only_the_library_a_file_landed_in_is_named_for_the_catalogue() {
    let root = kroma_testing::temp_dir("ingest-library");
    let file = root.path().join("Dune (2021)/Dune (2021).mkv.kremote");
    stub(&file);
    let other = LibraryDef { id: "other".into(), ..def(root.path()) };
    let defs = [def(root.path()), other];
    let (items, _, _) = index_under(&defs[..1], &[file]).unwrap();

    let named = libraries_of(&defs, &items);

    assert_eq!(named.len(), 1);
    assert_eq!(named[0].id, "lib");
    assert_eq!(named[0].kind, LibraryKind::Movies);
}
