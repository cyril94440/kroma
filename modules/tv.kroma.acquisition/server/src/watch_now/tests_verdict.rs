use kroma_module_sdk::db;
use kroma_module_sdk::domain::{RequestKind, RequestStatus};
use kroma_module_sdk::host::HostStorage as _;
use kroma_module_sdk::testing::blocking;
use serde_json::json;

use super::test_world::*;
use super::*;

async fn confirmed(world: &World, imported: &Imported) -> bool {
    let host = world.host.clone();
    let confirmation = Confirmation {
        download_id: imported.download_id.clone(),
    };
    blocking(move || confirm(&host, &confirmation).unwrap()).await
}

async fn rejected(world: &World, download_id: &str, reason: &str) -> bool {
    let host = world.host.clone();
    let rejection = Rejection {
        download_id: download_id.into(),
        reason: reason.into(),
    };
    blocking(move || reject(&host, &rejection).unwrap()).await
}

async fn imported(world: &World, ask: Ask) -> Imported {
    match run(world, ask).await {
        Reply::Imported(imported) => imported,
        other => panic!("{other:?}"),
    }
}

fn open_again(world: &World, request: &str) {
    world
        .host
        .db()
        .get()
        .unwrap()
        .execute("UPDATE requests SET status = 'approved' WHERE id = ?1", [request])
        .unwrap();
}

fn status_of(world: &World, request: &str) -> RequestStatus {
    let conn = world.host.db().get().unwrap();
    db::get_request(&conn, request).unwrap().unwrap().status
}

#[tokio::test]
async fn confirming_is_what_marks_the_request_available_and_records_the_import() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;
    assert_eq!(status_of(&world, "r1"), RequestStatus::Approved);

    assert!(confirmed(&world, &placed).await);

    assert_eq!(status_of(&world, "r1"), RequestStatus::Available);
    assert_eq!(world.engine.lock().unwrap().marked, [placed.download_id.clone()]);
    assert!(provisional::get(world.host.store(), &placed.download_id).unwrap().is_none());
    assert!(!confirmed(&world, &placed).await, "a second confirmation finds nothing");
}

#[tokio::test]
async fn a_rejected_fake_is_never_announced_as_available() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;

    assert!(rejected(&world, &placed.download_id, "fake").await);

    assert_eq!(status_of(&world, "r1"), RequestStatus::Approved);
    assert!(world.host.notifications().is_empty());
    assert!(world.engine.lock().unwrap().marked.is_empty());
}

#[tokio::test]
async fn the_import_pass_leaves_a_provisional_download_to_the_core() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;
    let host = world.host.clone();

    let summary = blocking(move || crate::import::import_pass(&host, &|_| {}).unwrap()).await;

    assert_eq!(summary.imported, 0);
    assert!(world.engine.lock().unwrap().marked.is_empty());
    assert!(provisional::get(world.host.store(), &placed.download_id).unwrap().is_some());
}

#[tokio::test]
async fn a_download_the_import_pass_is_importing_is_not_imported_twice() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;
    provisional::forget(world.host.store(), &placed.download_id).unwrap();
    let _held = crate::import::hold_for_test(world.host.store(), &placed.download_id);
    let host = world.host.clone();

    let summary = blocking(move || crate::import::import_pass(&host, &|_| {}).unwrap()).await;

    assert_eq!(summary.imported, 0);
}

#[tokio::test]
async fn an_unanswered_import_is_confirmed_after_the_grace_and_not_before() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;
    let host = world.host.clone();
    let early = blocking(move || reconcile(&host, &|_| {}).unwrap()).await;
    world
        .host
        .store()
        .get()
        .unwrap()
        .execute(
            "UPDATE provisional_imports SET created_at = created_at - ?1",
            [provisional::GRACE_MS + 1],
        )
        .unwrap();
    let host = world.host.clone();

    let late = blocking(move || reconcile(&host, &|_| {}).unwrap()).await;

    assert_eq!((early, late), (0, 1));
    assert_eq!(status_of(&world, "r1"), RequestStatus::Available);
    assert_eq!(world.engine.lock().unwrap().marked, [placed.download_id]);
}

#[tokio::test]
async fn a_rejection_takes_out_only_the_files_this_import_wrote_and_nothing_the_caller_names() {
    let world = world(RequestKind::Show, &[SHOW_E01, SHOW_PACK], &[SHOW_E01, SHOW_PACK]).await;
    open_episode_request(&world.host);
    let first = imported(&world, episode_ask()).await;
    assert!(confirmed(&world, &first).await);
    let kept = files_under(&world.library);
    assert_eq!(kept.len(), 1);
    open_again(&world, "r2");
    let mut second_ask = episode_ask();
    second_ask.episode = Some(2);
    second_ask.exclude = vec![format!("{SHOW_E01}|idx1")];
    open_second_episode(&world.host);

    let pack = imported(&world, second_ask).await;
    assert_eq!(pack.release.title, SHOW_PACK);
    assert_eq!(pack.paths.len(), 2, "E01 existed and is not listed: {:?}", pack.paths);
    assert!(!pack.paths.contains(&kept[0]));
    let host = world.host.clone();
    let rejection: Rejection = serde_json::from_value(json!({
        "download_id": pack.download_id, "reason": "fake", "paths": [kept[0]]
    }))
    .unwrap();
    assert!(blocking(move || reject(&host, &rejection).unwrap()).await);

    assert_eq!(files_under(&world.library), kept, "the earlier episode survived");
}

#[tokio::test]
async fn a_rejection_with_no_record_undoes_nothing() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    let placed = imported(&world, movie_ask()).await;
    let before = files_under(&world.library);

    assert!(!rejected(&world, "dl-unknown", "fake").await);

    assert_eq!(files_under(&world.library), before);
    assert!(provisional::get(world.host.store(), &placed.download_id).unwrap().is_some());
}

#[tokio::test]
async fn a_title_the_library_already_holds_is_answered_present_without_a_grab() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world
        .host
        .db()
        .get()
        .unwrap()
        .execute_batch(
            "INSERT INTO libraries (id,name,kind,path,added_at) VALUES ('hidden','H','movies','/h','t');
             INSERT INTO items (id,kind,title,container,library,added_at) VALUES ('i1','movie','Dune','mkv','hidden','t');
             INSERT INTO metadata_core (subject_kind,subject_id,tmdb_id,updated_at) VALUES ('item','i1',438631,0);",
        )
        .unwrap();

    let reply = run(&world, movie_ask()).await;

    assert_eq!(reply, Reply::Present { request_id: None });
    assert!(offered_titles(&world).is_empty());
    assert!(files_under(&world.library).is_empty());
}

#[tokio::test]
async fn a_file_already_at_the_destination_is_answered_present_and_never_removed() {
    let world = world(RequestKind::Movie, &[DUNE_1080, DUNE_720], &[DUNE_1080, DUNE_720]).await;
    open_movie_request(&world.host);
    let first = imported(&world, movie_ask()).await;
    assert!(confirmed(&world, &first).await);
    let kept = files_under(&world.library);
    open_again(&world, "r1");

    let again = run(&world, movie_ask()).await;

    assert!(matches!(again, Reply::Present { .. }), "{again:?}");
    assert_eq!(files_under(&world.library), kept);
}

#[tokio::test]
async fn a_release_the_engine_cannot_settle_is_blocklisted_when_deterministic_and_the_next_is_taken() {
    let world = world(RequestKind::Movie, &[DUNE_1080, DUNE_720], &[DUNE_1080, DUNE_720]).await;
    open_movie_request(&world.host);
    world.engine.lock().unwrap().unusable = vec![(DUNE_1080.into(), true)];

    let placed = imported(&world, movie_ask()).await;

    assert_eq!(placed.release.title, DUNE_720);
    let blocked = crate::rules::blocklist::for_title(world.host.store(), RequestKind::Movie, 438_631).unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].release_title, DUNE_1080);
}

#[tokio::test]
async fn a_release_that_merely_timed_out_is_skipped_without_blame() {
    let world = world(RequestKind::Movie, &[DUNE_1080, DUNE_720], &[DUNE_1080, DUNE_720]).await;
    open_movie_request(&world.host);
    world.engine.lock().unwrap().unusable = vec![(DUNE_1080.into(), false)];

    let placed = imported(&world, movie_ask()).await;

    assert_eq!(placed.release.title, DUNE_720);
    assert!(crate::rules::blocklist::for_title(world.host.store(), RequestKind::Movie, 438_631)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn when_every_release_is_unusable_the_answer_is_requested_not_an_error() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world.engine.lock().unwrap().unusable = vec![(DUNE_1080.into(), true)];

    let reply = run(&world, movie_ask()).await;

    assert_eq!(reply, Reply::Requested { request_id: "r1".into() });
}

#[tokio::test]
async fn a_download_already_on_its_way_is_not_grabbed_a_second_time() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world
        .host
        .db()
        .get()
        .unwrap()
        .execute(
            "INSERT INTO downloads (id, kind, tmdb_id, release_title, magnet_or_url, status, grabbed_at, progress, client_id, client_ref) \
             VALUES ('live', 'movie', 438631, 'Dune.live', 'magnet:?x', 'downloading', 1, 0.2, 'c', 'r')",
            [],
        )
        .unwrap();

    let reply = run(&world, movie_ask()).await;

    assert_eq!(reply, Reply::Requested { request_id: "r1".into() });
    assert!(offered_titles(&world).is_empty());
}

#[tokio::test]
async fn a_user_who_may_not_skip_the_queue_cannot_ask() {
    let world = world(RequestKind::Movie, &[DUNE_1080], &[DUNE_1080]).await;
    open_movie_request(&world.host);
    world
        .host
        .db()
        .get()
        .unwrap()
        .execute("UPDATE users SET permissions = '[\"playback\"]'", [])
        .unwrap();
    let host = world.host.clone();
    let ask = movie_ask();

    let refused = blocking(move || watch_now(&host, &ask).is_err()).await;

    assert!(refused);
    assert!(offered_titles(&world).is_empty());
}
