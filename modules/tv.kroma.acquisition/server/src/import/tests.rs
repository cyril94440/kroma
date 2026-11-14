use std::sync::{Arc, Mutex};

use axum::Json;
use axum::extract::State;
use axum::routing::post;
use serde_json::{Value, json};

use kroma_module_sdk::host::testing::StubHost;

use super::*;
use crate::peers::downloads::DOWNLOAD_DB;

type Moves = Arc<Mutex<Vec<(String, String, String)>>>;

fn season_row(id: &str, save_path: &Path) -> Value {
    json!({
        "id": id,
        "kind": "season",
        "title": "The Sopranos",
        "season": 1,
        "release_title": "The.Sopranos.S01.MULTi.VFF.1080p.Bluray.HDLight.AV1.10bit-RYO.FRENCH",
        "status": "completed",
        "save_path": save_path.to_string_lossy(),
    })
}

fn ledger(rows: Vec<Value>) -> axum::Router<Moves> {
    let completed = move || {
        let rows = rows.clone();
        async move { Json(Ok::<_, String>(rows)) }
    };
    let set_status = |State(moves): State<Moves>, Json(req): Json<Value>| async move {
        let text = |key: &str| req[key].as_str().unwrap_or_default().to_string();
        moves
            .lock()
            .unwrap()
            .push((text("id"), text("status"), text("error")));
        Json(Ok::<_, String>(true))
    };
    axum::Router::new()
        .route("/_port/tv.kroma.torrents/db/completed", post(completed))
        .route("/_port/tv.kroma.torrents/db/set-status", post(set_status))
}

#[tokio::test]
async fn a_download_whose_folder_is_gone_is_marked_removed_and_says_why() {
    let downloads = kroma_module_sdk::testing::temp_dir("import-gone");
    let empty = downloads.path().join("54ed71fbe0318163");
    std::fs::create_dir(&empty).unwrap();
    let rows = vec![
        season_row(
            "68165ad60353d090",
            &downloads.path().join("68165ad60353d090"),
        ),
        season_row("54ed71fbe0318163", &empty),
    ];
    let moves = Moves::default();
    let resolve = kroma_module_sdk::testing::serve(ledger(rows), moves.clone()).await;
    let (base, token) = resolve().expect("the fake ledger is up");
    let host = StubHost::with_core(kroma_module_sdk::db::testing::temp_pool("import-gone"))
        .with_point(DOWNLOAD_DB, None, &base, &token);
    crate::rules::schema::install(host.store());

    let summary = kroma_module_sdk::testing::blocking(move || import_pass(&host, &|_| {}))
        .await
        .unwrap();

    assert_eq!(summary.failed, 2);
    let moves = moves.lock().unwrap();
    let gone = &moves[0];
    assert_eq!(
        (gone.0.as_str(), gone.1.as_str()),
        ("68165ad60353d090", "removed")
    );
    assert!(
        gone.2.contains("the downloaded files are gone"),
        "{}",
        gone.2
    );
    let empty = &moves[1];
    assert_eq!(
        (empty.0.as_str(), empty.1.as_str()),
        ("54ed71fbe0318163", "completed")
    );
    assert!(empty.2.contains("no video file found"), "{}", empty.2);
}

mod stubs {
    use kroma_module_sdk::host::{HostCtx, LibraryFolders};
    use kroma_module_sdk::primitives::remote_file::{self, RemoteFile};

    use super::*;

    fn stub(folder: &Path, rel: &str, name: &str, size: u64) {
        let file = RemoteFile {
            source: "alldebrid".into(),
            reference: format!("7:{name}"),
            name: name.into(),
            size,
        };
        remote_file::write(&folder.join(rel), &file).unwrap();
    }

    fn host_with_library(kind: &str, library: &Path) -> StubHost {
        StubHost::with_db("import-stubs").with_libraries(vec![LibraryFolders {
            id: "lib".into(),
            kind: kind.into(),
            name: "Library".into(),
            folders: vec![library.to_string_lossy().into_owned()],
        }])
    }

    fn row(kind: &str, save_path: &Path, rels: &[&str]) -> DownloadRow {
        DownloadRow {
            id: "d1".into(),
            kind: kind.into(),
            title: Some("The Movie".into()),
            year: Some(2020),
            season: Some(1),
            release_title: "The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP".into(),
            status: "completed".into(),
            save_path: Some(save_path.to_string_lossy().into_owned()),
            content_rel_paths: rels.iter().map(ToString::to_string).collect(),
            ..DownloadRow::default()
        }
    }

    fn names_under(root: &Path) -> Vec<String> {
        let mut found: Vec<String> = walkdir::WalkDir::new(root)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .map(|entry| entry.path().strip_prefix(root).unwrap().display().to_string())
            .collect();
        found.sort();
        found
    }

    #[test]
    fn a_movie_stub_lands_in_the_library_named_by_the_template_with_the_stub_extension() {
        let scratch = kroma_module_sdk::testing::temp_dir("import-stub-movie");
        let downloads = scratch.path().join("downloads/7");
        let library = scratch.path().join("movies");
        stub(&downloads, "The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.mkv.kremote", "The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.mkv", 9_000_000_000);
        std::fs::write(downloads.join("The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.en.srt"), "1\n").unwrap();
        let host = host_with_library("movies", &library);
        let row = row("movie", &downloads, &["The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.mkv.kremote", "The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.en.srt"]);

        let placed = import_one(&host, &row).unwrap();

        assert_eq!(placed.len(), 1);
        assert!(placed[0].ends_with(".kremote"), "{placed:?}");
        let landed = remote_file::read(Path::new(&placed[0])).unwrap();
        assert_eq!(landed.name, "The.Movie.2020.2160p.WEB-DL.DDP5.1.HEVC-GRP.mkv");
        assert_eq!(landed.container().as_deref(), Some("mkv"));
        let names = names_under(&library);
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names.iter().any(|n| n.ends_with(".srt")), "the subtitle follows the stub: {names:?}");
        assert!(
            names.iter().any(|n| n.contains("The Movie (2020)") && n.contains("2160p")),
            "quality tokens come from the real name: {names:?}"
        );
    }

    #[test]
    fn a_season_pack_of_stubs_imports_every_episode_with_its_own_subtitles() {
        let scratch = kroma_module_sdk::testing::temp_dir("import-stub-season");
        let downloads = scratch.path().join("downloads/7");
        let library = scratch.path().join("shows");
        let mut rels = Vec::new();
        for episode in 1..=3 {
            let real = format!("The.Show.S01E0{episode}.1080p.WEB-DL-GRP.mkv");
            stub(&downloads, &format!("Pack/{real}.kremote"), &real, 2_000_000_000);
            std::fs::write(downloads.join(format!("Pack/The.Show.S01E0{episode}.1080p.WEB-DL-GRP.en.srt")), "1\n").unwrap();
            rels.push(format!("Pack/{real}.kremote"));
            rels.push(format!("Pack/The.Show.S01E0{episode}.1080p.WEB-DL-GRP.en.srt"));
        }
        let host = host_with_library("shows", &library);
        let rels: Vec<&str> = rels.iter().map(String::as_str).collect();
        let row = row("season", &downloads, &rels);

        let placed = import_one(&host, &row).unwrap();

        assert_eq!(placed.len(), 3);
        let names = names_under(&library);
        assert_eq!(names.iter().filter(|n| n.ends_with(".kremote")).count(), 3, "{names:?}");
        assert_eq!(names.iter().filter(|n| n.ends_with(".srt")).count(), 3, "{names:?}");
    }

    #[test]
    fn a_sample_stub_beside_the_feature_is_not_imported() {
        let scratch = kroma_module_sdk::testing::temp_dir("import-stub-sample");
        let downloads = scratch.path().join("downloads/7");
        let library = scratch.path().join("movies");
        stub(&downloads, "sample.kremote", "The.Movie.2020.sample.mkv", 99_000_000_000);
        stub(&downloads, "feature.kremote", "The.Movie.2020.1080p-GRP.mkv", 4_000_000_000);
        let host = host_with_library("movies", &library);
        let row = row("movie", &downloads, &[]);

        let placed = import_one(&host, &row).unwrap();

        let landed = remote_file::read(Path::new(&placed[0])).unwrap();
        assert_eq!(landed.name, "The.Movie.2020.1080p-GRP.mkv");
    }

    #[test]
    fn the_stub_a_hardlink_keeps_working_after_the_download_folder_is_deleted() {
        let scratch = kroma_module_sdk::testing::temp_dir("import-stub-drop");
        let downloads = scratch.path().join("downloads/7");
        let library = scratch.path().join("movies");
        stub(&downloads, "a.kremote", "The.Movie.2020.1080p-GRP.mkv", 4_000_000_000);
        let host = host_with_library("movies", &library);
        let row = row("movie", &downloads, &[]);
        let placed = import_one(&host, &row).unwrap();

        std::fs::remove_dir_all(&downloads).unwrap();

        assert_eq!(remote_file::read(Path::new(&placed[0])).unwrap().size, 4_000_000_000);
        let _ = host.data_dir();
    }
}
