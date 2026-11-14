use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::db::{self, NewRequest};
use kroma_module_sdk::domain::{EpisodeRef, RequestKind, RequestStatus};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::LibraryFolders;
use kroma_module_sdk::primitives::remote_file::{self, RemoteFile};
use kroma_module_sdk::testing::{blocking, serve, temp_dir, TempDir};
use serde_json::{json, Value};

use super::*;
use crate::peers::downloads::{DOWNLOAD_DB, DOWNLOAD_GRAB};
use crate::peers::indexers::{INDEXER_DB, INDEXER_SEARCH};
use crate::search::best::test_world::{monitor, monitored, release_json, want, wanted_row};

pub(super) const DUNE_1080: &str = "Dune.2021.1080p.BluRay.x264-GRP";
pub(super) const DUNE_720: &str = "Dune.2021.720p.WEB-DL.x264-GRP";
pub(super) const DUNE_CAM: &str = "Dune.2021.1080p.CAM.x264-GRP";
pub(super) const SHOW_E01: &str = "The.Show.S01E01.1080p.WEB-DL.x264-GRP";
pub(super) const SHOW_PACK: &str = "The.Show.S01.1080p.WEB-DL.x264-GRP";

#[derive(Default)]
pub(super) struct Engine {
    pub held: Vec<String>,
    pub capable: bool,
    pub scratch: PathBuf,
    pub offered: Vec<Value>,
    pub completed: Vec<Value>,
    pub statuses: Vec<(String, String)>,
    pub dropped: Vec<String>,
    pub discarded: Vec<String>,
    pub marked: Vec<String>,
    pub unusable: Vec<(String, bool)>,
}

pub(super) type Shared = Arc<Mutex<Engine>>;

pub(super) struct World {
    pub host: StubHost,
    pub engine: Shared,
    pub library: PathBuf,
    pub _scratch: TempDir,
}

pub(super) fn stub(folder: &Path, rel: &str, name: &str) {
    let file = RemoteFile {
        source: "alldebrid".into(),
        reference: format!("7:{name}"),
        name: name.into(),
        size: 2_000_000_000,
    };
    remote_file::write(&folder.join(rel), &file).unwrap();
}

pub(super) async fn instant(
    State(engine): State<Shared>,
    Json(body): Json<Value>,
) -> Json<Result<Value, String>> {
    let mut engine = engine.lock().unwrap();
    engine.offered.push(body.clone());
    let chosen = body["releases"].as_array().unwrap().iter().find(|r| {
        engine
            .held
            .iter()
            .any(|title| title == r["spec"]["release_title"].as_str().unwrap())
    });
    let Some(chosen) = chosen else {
        return Json(Ok(json!({ "status": "nothing" })));
    };
    let spec = &chosen["spec"];
    let title_of = spec["release_title"].as_str().unwrap().to_string();
    if let Some((_, deterministic)) = engine.unusable.iter().find(|(t, _)| *t == title_of).cloned() {
        return Json(Ok(json!({
            "status": "failed", "key": chosen["key"], "reason": "no video file",
            "deterministic": deterministic
        })));
    }
    let id = format!("dl{}", engine.completed.len() + 1);
    let folder = engine.scratch.join("downloads").join(&id);
    let title = spec["release_title"].as_str().unwrap();
    let rels: Vec<String> = if spec["kind"] == "season" {
        (1..=3)
            .map(|n| {
                let real = format!("The.Show.S01E0{n}.1080p.WEB-DL.x264-GRP.mkv");
                stub(&folder, &format!("Pack/{real}.kremote"), &real);
                format!("Pack/{real}.kremote")
            })
            .collect()
    } else {
        let real = format!("{title}.mkv");
        stub(&folder, &format!("{real}.kremote"), &real);
        vec![format!("{real}.kremote")]
    };
    let row = json!({
        "id": id, "kind": spec["kind"], "tmdb_id": spec["tmdb_id"], "title": spec["title"],
        "year": spec["year"], "season": spec["season"], "episodes": spec["episodes"],
        "release_title": title, "status": "completed", "save_path": folder.to_string_lossy(),
        "request_id": spec["request_id"], "magnet_or_url": spec["magnet_or_url"],
    });
    let mut with_files = row.clone();
    with_files["content_rel_paths"] = json!(rels);
    engine.completed.push(with_files);
    Json(Ok(
        json!({ "status": "grabbed", "key": chosen["key"], "download": row }),
    ))
}

pub(super) async fn world(kind: RequestKind, offering: &[&str], held: &[&str]) -> World {
    let scratch = temp_dir("watch-now");
    let library = scratch.path().join("library");
    std::fs::create_dir_all(&library).unwrap();
    let engine: Shared = Arc::new(Mutex::new(Engine {
        held: held.iter().map(ToString::to_string).collect(),
        capable: true,
        scratch: scratch.path().to_path_buf(),
        ..Engine::default()
    }));
    let releases: Vec<Value> = offering.iter().map(|t| release_json(t)).collect();
    let router: Router<Shared> = Router::new()
        .route("/_port/tv.kroma.torrents/grab/grab-instant", post(instant))
        .route(
            "/_port/tv.kroma.torrents/grab/instant-capable",
            post(|State(e): State<Shared>, Json(_): Json<Value>| async move { Json(e.lock().unwrap().capable) }),
        )
        .route(
            "/_port/tv.kroma.torrents/grab/drop-data",
            post(|State(e): State<Shared>, Json(b): Json<Value>| async move {
                let mut engine = e.lock().unwrap();
                engine.dropped.push(b["id"].as_str().unwrap().into());
                if b["discard"] == true {
                    engine.discarded.push(b["id"].as_str().unwrap().into());
                }
                Json(Ok::<_, String>(true))
            }),
        )
        .route(
            "/_port/tv.kroma.torrents/db/completed",
            post(|State(e): State<Shared>, Json(_): Json<Value>| async move {
                let engine = e.lock().unwrap();
                let rows: Vec<Value> = engine
                    .completed
                    .iter()
                    .filter(|r| !engine.marked.contains(&r["id"].as_str().unwrap().to_string()))
                    .cloned()
                    .collect();
                Json(Ok::<_, String>(rows))
            }),
        )
        .route(
            "/_port/tv.kroma.torrents/db/mark-imported",
            post(|State(e): State<Shared>, Json(b): Json<Value>| async move {
                e.lock().unwrap().marked.push(b["id"].as_str().unwrap().into());
                Json(Ok::<_, String>(()))
            }),
        )
        .route(
            "/_port/tv.kroma.torrents/db/set-status",
            post(|State(e): State<Shared>, Json(b): Json<Value>| async move {
                e.lock().unwrap().statuses.push((b["id"].as_str().unwrap().into(), b["status"].as_str().unwrap().into()));
                Json(Ok::<_, String>(true))
            }),
        )
        .route(
            "/_port/tv.kroma.indexer/db/enabled",
            post(|State(_): State<Shared>, Json(_): Json<Value>| async {
                Json(Ok::<_, String>(vec![json!({ "id": "idx1", "name": "Tracker", "kind": "torznab", "priority": 30, "enabled": true })]))
            }),
        )
        .route(
            "/_port/tv.kroma.indexer/db/get",
            post(|State(_): State<Shared>, Json(_): Json<Value>| async { Json(Ok::<_, String>(Value::Null)) }),
        )
        .route(
            "/_port/tv.kroma.indexer/db/note-result",
            post(|State(_): State<Shared>, Json(_): Json<Value>| async { Json(Ok::<_, String>(())) }),
        )
        .route(
            "/_port/tv.kroma.indexer/search/search",
            post(move |State(_): State<Shared>, Json(_): Json<Value>| {
                let releases = releases.clone();
                async move { Json(Ok::<_, String>(json!({ "releases": releases, "errors": [] }))) }
            }),
        );
    let resolve = serve(router, engine.clone()).await;
    let (base, token) = resolve().expect("the fakes are up");
    let library_kind = if kind == RequestKind::Movie {
        "movies"
    } else {
        "shows"
    };
    let host = StubHost::with_core(db::testing::temp_pool("acq-watch-now"))
        .with_setting("acqEnabled", json!(true))
        .with_setting("acqForbiddenKeywords", json!("cam"))
        .with_libraries(vec![LibraryFolders {
            id: "lib".into(),
            kind: library_kind.into(),
            name: "Library".into(),
            folders: vec![library.to_string_lossy().into_owned()],
        }])
        .with_point(DOWNLOAD_GRAB, None, &base, &token)
        .with_point(DOWNLOAD_DB, None, &base, &token)
        .with_point(INDEXER_DB, None, &base, &token)
        .with_point(INDEXER_SEARCH, None, &base, &token);
    crate::rules::schema::install(host.store());
    host.db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO users (id,email,username,password_hash,permissions,created_at) VALUES ('u1','a@b.c','alice','h','[\"requests.create\",\"requests.auto\"]','t')",
            [],
        )
        .unwrap();
    World {
        host,
        engine,
        library,
        _scratch: scratch,
    }
}

pub(super) fn open_movie_request(host: &StubHost) {
    let dune = monitored(RequestKind::Movie, 438_631, "Dune", Some(2021));
    monitor(host, &dune);
    db::insert_request(
        host.db(),
        &NewRequest {
            id: "r1".into(),
            kind: RequestKind::Movie,
            tmdb_id: 438_631,
            title: "Dune".into(),
            year: Some(2021),
            poster_url: None,
            seasons: None,
            episodes: None,
            status: RequestStatus::Approved,
            requested_by: None,
        },
        1,
    )
    .unwrap();
    let mut row = wanted_row("w1", &dune, None, None, "wanted");
    row.request_id = Some("r1".into());
    want(host, &[row]);
}

pub(super) fn open_episode_request(host: &StubHost) {
    let show = monitored(RequestKind::Show, 42, "The Show", Some(2020));
    monitor(host, &show);
    db::insert_request(
        host.db(),
        &NewRequest {
            id: "r2".into(),
            kind: RequestKind::Show,
            tmdb_id: 42,
            title: "The Show".into(),
            year: Some(2020),
            poster_url: None,
            seasons: None,
            episodes: Some(vec![
                EpisodeRef {
                    season: 1,
                    episode: 1,
                },
                EpisodeRef {
                    season: 1,
                    episode: 2,
                },
            ]),
            status: RequestStatus::Approved,
            requested_by: None,
        },
        1,
    )
    .unwrap();
    let mut row = wanted_row("w2", &show, Some(1), Some(1), "wanted");
    row.request_id = Some("r2".into());
    want(host, &[row]);
}

pub(super) fn movie_ask() -> Ask {
    Ask {
        kind: "movie".into(),
        tmdb_id: 438_631,
        user_id: "u1".into(),
        ..Ask::default()
    }
}

pub(super) fn episode_ask() -> Ask {
    Ask {
        kind: "show".into(),
        tmdb_id: 42,
        season: Some(1),
        episode: Some(1),
        user_id: "u1".into(),
        ..Ask::default()
    }
}

pub(super) async fn run(world: &World, ask: Ask) -> Reply {
    let host = world.host.clone();
    blocking(move || watch_now(&host, &ask).unwrap()).await
}

pub(super) fn files_under(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().display().to_string())
        .collect()
}

pub(super) fn offered_titles(world: &World) -> Vec<String> {
    world
        .engine
        .lock()
        .unwrap()
        .offered
        .iter()
        .flat_map(|call| call["releases"].as_array().unwrap().clone())
        .map(|r| r["spec"]["release_title"].as_str().unwrap().to_string())
        .collect()
}


pub(super) fn open_second_episode(host: &StubHost) {
    let show = monitored(RequestKind::Show, 42, "The Show", Some(2020));
    let mut row = wanted_row("w3", &show, Some(1), Some(2), "wanted");
    row.request_id = Some("r2".into());
    want(host, &[row]);
}
