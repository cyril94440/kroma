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

const DUNE_1080: &str = "Dune.2021.1080p.BluRay.x264-GRP";
const DUNE_720: &str = "Dune.2021.720p.WEB-DL.x264-GRP";
const DUNE_CAM: &str = "Dune.2021.1080p.CAM.x264-GRP";
const SHOW_E01: &str = "The.Show.S01E01.1080p.WEB-DL.x264-GRP";
const SHOW_PACK: &str = "The.Show.S01.1080p.WEB-DL.x264-GRP";

#[derive(Default)]
struct Engine {
    held: Vec<String>,
    capable: bool,
    scratch: PathBuf,
    offered: Vec<Value>,
    completed: Vec<Value>,
    statuses: Vec<(String, String)>,
    dropped: Vec<String>,
    marked: Vec<String>,
}

type Shared = Arc<Mutex<Engine>>;

struct World {
    host: StubHost,
    engine: Shared,
    library: PathBuf,
    _scratch: TempDir,
}

fn stub(folder: &Path, rel: &str, name: &str) {
    let file = RemoteFile {
        source: "alldebrid".into(),
        reference: format!("7:{name}"),
        name: name.into(),
        size: 2_000_000_000,
    };
    remote_file::write(&folder.join(rel), &file).unwrap();
}

async fn instant(
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
        "request_id": spec["request_id"],
    });
    let mut with_files = row.clone();
    with_files["content_rel_paths"] = json!(rels);
    engine.completed.push(with_files);
    Json(Ok(
        json!({ "status": "grabbed", "key": chosen["key"], "download": row }),
    ))
}

async fn world(kind: RequestKind, offering: &[&str], held: &[&str]) -> World {
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
                e.lock().unwrap().dropped.push(b["id"].as_str().unwrap().into());
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
    World {
        host,
        engine,
        library,
        _scratch: scratch,
    }
}

fn open_movie_request(host: &StubHost) {
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

fn open_episode_request(host: &StubHost) {
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
            episodes: Some(vec![EpisodeRef {
                season: 1,
                episode: 1,
            }]),
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

fn movie_ask() -> Ask {
    Ask {
        kind: "movie".into(),
        tmdb_id: 438_631,
        user_id: "u1".into(),
        ..Ask::default()
    }
}

fn episode_ask() -> Ask {
    Ask {
        kind: "show".into(),
        tmdb_id: 42,
        season: Some(1),
        episode: Some(1),
        user_id: "u1".into(),
        ..Ask::default()
    }
}

async fn run(world: &World, ask: Ask) -> Reply {
    let host = world.host.clone();
    blocking(move || watch_now(&host, &ask).unwrap()).await
}

fn files_under(root: &Path) -> Vec<String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().display().to_string())
        .collect()
}

fn offered_titles(world: &World) -> Vec<String> {
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

#[tokio::test]
async fn a_movie_the_client_holds_is_imported_at_once_and_handed_back_with_its_claimed_height() {
    let world = world(RequestKind::Movie, &[DUNE_720, DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.request_id, "r1");
    assert_eq!(imported.release.title, DUNE_1080);
    assert_eq!(imported.release.claimed_height, Some(1080));
    assert!(
        imported.primary_path.ends_with(".kremote"),
        "{}",
        imported.primary_path
    );
    assert!(Path::new(&imported.primary_path).starts_with(&world.library));
    assert_eq!(world.engine.lock().unwrap().marked, [imported.download_id]);
}

#[tokio::test]
async fn the_best_scored_release_the_client_holds_wins_over_a_better_one_it_does_not() {
    let world = world(RequestKind::Movie, &[DUNE_1080, DUNE_720], &[DUNE_720]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, DUNE_720);
    assert_eq!(offered_titles(&world), [DUNE_1080, DUNE_720]);
}

#[tokio::test]
async fn nothing_held_leaves_the_approved_request_and_says_requested() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Requested {
            request_id: "r1".into()
        }
    );
    let conn = world.host.db().get().unwrap();
    assert_eq!(
        db::get_request(&conn, "r1").unwrap().unwrap().status,
        RequestStatus::Approved
    );
    assert!(files_under(&world.library).is_empty());
}

#[tokio::test]
async fn a_release_the_rules_refuse_is_never_offered_even_when_it_is_the_only_one_held() {
    let world = world(RequestKind::Movie, &[DUNE_CAM, DUNE_1080], &[DUNE_CAM]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Requested {
            request_id: "r1".into()
        }
    );
    assert!(
        !offered_titles(&world).iter().any(|t| t == DUNE_CAM),
        "{:?}",
        offered_titles(&world)
    );
    assert!(files_under(&world.library).is_empty());
}

#[tokio::test]
async fn nothing_the_rules_accept_is_unavailable_with_the_request_kept() {
    let world = world(RequestKind::Movie, &[DUNE_CAM], &[DUNE_CAM]).await;
    open_movie_request(&world.host);

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Unavailable {
            reason: Why::NoRelease,
            request_id: Some("r1".into())
        }
    );
}

#[tokio::test]
async fn a_client_that_cannot_say_what_it_holds_makes_the_title_unavailable_before_any_search() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world.engine.lock().unwrap().capable = false;

    let reply = run(&world, movie_ask()).await;

    assert_eq!(
        reply,
        Reply::Unavailable {
            reason: Why::NoEngine,
            request_id: None
        }
    );
    assert!(offered_titles(&world).is_empty());
}

#[tokio::test]
async fn a_release_the_core_turned_down_is_not_offered_again() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let mut ask = movie_ask();
    ask.exclude = vec![format!("{DUNE_1080}|idx1")];

    let reply = run(&world, ask).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, DUNE_720);
    assert_eq!(offered_titles(&world), [DUNE_720]);
}

#[tokio::test]
async fn an_episode_with_no_instant_release_falls_back_to_the_season_pack_and_returns_that_episode()
{
    let world = world(RequestKind::Show, &[SHOW_E01, SHOW_PACK], &[SHOW_PACK]).await;
    open_episode_request(&world.host);

    let reply = run(&world, episode_ask()).await;

    let Reply::Imported(imported) = reply else {
        panic!("{reply:?}")
    };
    assert_eq!(imported.release.title, SHOW_PACK);
    assert_eq!(
        imported.paths.len(),
        3,
        "every episode of the pack becomes a stub"
    );
    assert!(
        imported.primary_path.contains("S01E01"),
        "{}",
        imported.primary_path
    );
}

#[tokio::test]
async fn a_second_ask_for_an_open_request_makes_no_second_one() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[]).await;
    open_movie_request(&world.host);

    run(&world, movie_ask()).await;
    run(&world, movie_ask()).await;

    let conn = world.host.db().get().unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM requests", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn a_rejected_release_is_blocklisted_its_files_leave_and_the_download_is_released() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let Reply::Imported(imported) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };
    let host = world.host.clone();
    let rejection = Rejection {
        download_id: imported.download_id.clone(),
        release_title: imported.release.title.clone(),
        kind: "movie".into(),
        tmdb_id: 438_631,
        reason: "the file runs 4 minutes".into(),
        paths: imported.paths.clone(),
        request_id: Some(imported.request_id.clone()),
        season: None,
    };

    assert!(blocking(move || reject(&host, &rejection).unwrap()).await);

    assert!(files_under(&world.library).is_empty());
    let engine = world.engine.lock().unwrap();
    assert!(engine
        .statuses
        .contains(&(imported.download_id.clone(), "removed".into())));
    assert_eq!(engine.dropped, [imported.download_id]);
    drop(engine);
    let blocked =
        crate::rules::blocklist::for_title(world.host.store(), RequestKind::Movie, 438_631)
            .unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].release_title, DUNE_1080);
    assert!(blocked[0].reason.starts_with("watch now: "));
    let conn = world.host.db().get().unwrap();
    assert_eq!(
        db::get_request(&conn, "r1").unwrap().unwrap().status,
        RequestStatus::Approved
    );
    assert_eq!(
        db::wanted_for_request(&conn, "r1").unwrap()[0].status,
        "wanted"
    );
}

#[tokio::test]
async fn after_a_rejection_the_next_search_refuses_the_blocklisted_release_and_offers_the_next() {
    let world = world(
        RequestKind::Movie,
        &[DUNE_1080, DUNE_720],
        &[DUNE_1080, DUNE_720],
    )
    .await;
    open_movie_request(&world.host);
    let Reply::Imported(first) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };
    let host = world.host.clone();
    let rejection = Rejection {
        download_id: first.download_id,
        release_title: first.release.title,
        kind: "movie".into(),
        tmdb_id: 438_631,
        reason: "fake".into(),
        paths: first.paths,
        request_id: Some(first.request_id),
        season: None,
    };
    blocking(move || reject(&host, &rejection).unwrap()).await;

    let Reply::Imported(second) = run(&world, movie_ask()).await else {
        panic!("not imported")
    };

    assert_eq!(second.release.title, DUNE_720);
}

#[test]
fn a_show_is_asked_for_one_episode_at_a_time() {
    let ask = Ask {
        kind: "show".into(),
        tmdb_id: 42,
        ..Ask::default()
    };

    assert!(asked_unit(RequestKind::Show, &ask).is_err());
    assert_eq!(asked_unit(RequestKind::Movie, &ask).unwrap(), None);
}

#[test]
fn the_reply_crosses_as_a_status_tag_with_snake_case_fields() {
    let imported = serde_json::to_value(Reply::Imported(Imported {
        request_id: "r".into(),
        download_id: "d".into(),
        release: Release {
            key: "k".into(),
            title: "T".into(),
            claimed_height: Some(1080),
        },
        paths: vec!["/a".into()],
        primary_path: "/a".into(),
    }))
    .unwrap();

    assert_eq!(
        imported,
        json!({
            "status": "imported", "request_id": "r", "download_id": "d",
            "release": { "key": "k", "title": "T", "claimed_height": 1080 },
            "paths": ["/a"], "primary_path": "/a"
        })
    );
    assert_eq!(
        serde_json::to_value(Reply::Requested {
            request_id: "r".into()
        })
        .unwrap(),
        json!({ "status": "requested", "request_id": "r" })
    );
    assert_eq!(
        serde_json::to_value(Reply::Unavailable {
            reason: Why::NoIndexer,
            request_id: None
        })
        .unwrap(),
        json!({ "status": "unavailable", "reason": "noIndexer", "request_id": null })
    );
}

#[test]
fn an_ask_reads_from_the_core_s_json_and_tolerates_missing_fields() {
    let ask: Ask =
        serde_json::from_value(json!({ "kind": "movie", "tmdb_id": 1, "user_id": "u" })).unwrap();

    assert!(ask.exclude.is_empty());
    assert_eq!(ask.budget_ms, None);
}
