use super::*;

fn user(permissions: Vec<Permission>) -> User {
    User {
        id: "u1".into(),
        email: "a@t.dev".into(),
        username: "a".into(),
        avatar_url: None,
        language: None,
        audio_language: None,
        subtitle_language: None,
        permissions,
        libraries: crate::model::LibraryScope::All,
        created_at: "now".into(),
        has_pin: false,
    }
}

#[test]
fn only_a_requester_whose_title_needs_no_approval_may_watch_now() {
    assert!(may_watch_now(&user(vec![
        Permission::RequestsCreate,
        Permission::RequestsAuto
    ])));
    assert!(may_watch_now(&user(vec![
        Permission::RequestsCreate,
        Permission::RequestsManage
    ])));
    assert!(!may_watch_now(&user(vec![Permission::RequestsCreate])));
    assert!(!may_watch_now(&user(vec![Permission::RequestsAuto])));
    assert!(!may_watch_now(&user(vec![Permission::RequestsManage])));
    assert!(!may_watch_now(&user(Vec::new())));
}

#[test]
fn one_title_has_one_lock_and_two_titles_have_two() {
    let flights = Flights::default();
    let movie = Scope {
        kind: RequestKind::Movie,
        tmdb_id: 1,
        season: None,
        episode: None,
    };
    let episode = Scope {
        kind: RequestKind::Show,
        tmdb_id: 1,
        season: Some(1),
        episode: Some(1),
    };

    let same = (
        flights.lock_for(&movie.key()),
        flights.lock_for(&movie.key()),
    );
    let other = flights.lock_for(&episode.key());

    assert!(Arc::ptr_eq(&same.0, &same.1));
    assert!(!Arc::ptr_eq(&same.0, &other));
}

#[tokio::test]
async fn a_second_asker_waits_for_the_first_instead_of_running_beside_it() {
    let flights = Flights::default();
    let first = flights.lock_for("k");
    let held = first.lock().await;

    let second = flights.lock_for("k");

    assert!(second.try_lock().is_err());
    drop(held);
    assert!(second.try_lock().is_ok());
}

#[test]
fn the_scope_names_an_episode_only_when_it_has_both_numbers() {
    let whole = Scope {
        kind: RequestKind::Show,
        tmdb_id: 1,
        season: Some(1),
        episode: None,
    };
    let one = Scope {
        kind: RequestKind::Show,
        tmdb_id: 1,
        season: Some(2),
        episode: Some(3),
    };

    assert_eq!(whole.unit(), None);
    assert_eq!(one.unit(), Some((2, 3)));
}

#[test]
fn the_acquisition_modules_reasons_map_onto_the_ones_clients_know() {
    assert_eq!(reason_of(Why::NoIndexer), Reason::NoIndexer);
    assert_eq!(reason_of(Why::NoRelease), Reason::NoRelease);
    assert_eq!(reason_of(Why::Other), Reason::NoEngine);
}

#[test]
fn a_title_nobody_holds_any_more_leaves_the_map_the_next_time_one_is_asked_for() {
    let flights = Flights::default();
    for n in 0..50 {
        drop(flights.lock_for(&format!("title-{n}")));
    }
    let kept = flights.lock_for("kept");

    let _ = flights.lock_for("another");

    assert_eq!(flights.held(), 2, "only the held lock and the one just asked for remain");
    drop(kept);
}
