use std::path::Path;
use std::sync::Arc;

use serde_json::json;

use super::tests::{cached, node, payload, setup};
use crate::cache::{Availability, Cache};
use crate::fake_alldebrid::{host_of, scripted, Fake, Resp};

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_server_that_ignores_ranges_still_delivers_the_file() {
    let body = payload(150_000);
    let fake = scripted(
        json!([node("a.mkv", body.len())]),
        vec![("a.mkv", body.clone())],
        false,
    );
    let (_dir, alldebrid) = setup("ad-cache-norange", &fake);

    let landed = cached(&alldebrid, "a.mkv").await;

    let Availability::Cached(path) = landed else {
        panic!("{landed:?}")
    };
    assert_eq!(std::fs::read(path).unwrap(), body);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn an_expired_link_is_unlocked_again() {
    let body = payload(100_000);
    let served = body.clone();
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = hits.clone();
    let fake = Fake::start(move |req| match req.path.as_str() {
        "/v4/magnet/files" => {
            Resp::data(json!({ "magnets": [{ "id": "7", "files": [node("a.mkv", 100_000)] }] }))
        }
        "/v4/link/unlock" => Resp::data(
            json!({ "link": format!("http://{}/dl/a.mkv", host_of(req)), "filesize": 100_000 }),
        ),
        _ if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 => {
            Resp::raw(403, Vec::new())
        }
        _ => Resp::bytes(req, &served, true),
    });
    let (_dir, alldebrid) = setup("ad-cache-relock", &fake);

    let landed = cached(&alldebrid, "a.mkv").await;

    let Availability::Cached(path) = landed else {
        panic!("{landed:?}")
    };
    assert_eq!(std::fs::read(path).unwrap(), body);
    assert_eq!(fake.calls_to("/v4/link/unlock").len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_failing_server_surfaces_the_reason_and_leaves_nothing_complete() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4/magnet/files" => {
            Resp::data(json!({ "magnets": [{ "id": "7", "files": [node("a.mkv", 5000)] }] }))
        }
        "/v4/link/unlock" => Resp::data(
            json!({ "link": format!("http://{}/dl/a.mkv", host_of(req)), "filesize": 5000 }),
        ),
        _ => Resp::raw(500, b"boom".to_vec()),
    });
    let (dir, alldebrid) = setup("ad-cache-fail", &fake);

    let landed = cached(&alldebrid, "a.mkv").await;

    let Availability::Failed(reason) = landed else {
        panic!("{landed:?}")
    };
    assert!(reason.contains("HTTP 500"), "{reason}");
    assert!(!dir.path().join("cache/7/a.mkv").exists());
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_hostile_name_lands_inside_the_cache_and_nowhere_else() {
    let body = payload(500);
    let tree = json!([{ "n": "..", "e": [{ "n": "..", "e": [node("evil.mkv", 500)] }] }]);
    let fake = scripted(tree, vec![("evil.mkv", body)], true);
    let (dir, alldebrid) = setup("ad-cache-hostile", &fake);

    let landed = cached(&alldebrid, "_/_/evil.mkv").await;

    let Availability::Cached(path) = landed else {
        panic!("{landed:?}")
    };
    assert_eq!(path, dir.path().join("cache/7/_/_/evil.mkv"));
    assert!(!dir.path().join("evil.mkv").exists());
}

#[test]
fn the_cache_only_names_paths_it_would_have_planned() {
    let cache = Cache::new(Path::new("/m"));

    assert!(cache.path_of("7", "a/b.mkv").is_ok());
    assert!(cache.path_of("7", "../b.mkv").is_err());
    assert!(cache.path_of("7", "/abs").is_err());
    assert!(cache.path_of("../7", "b.mkv").is_err());
    assert!(cache.path_of("", "b.mkv").is_err());
}

#[test]
fn a_cached_path_is_only_answered_for_a_complete_file_of_the_right_size() {
    let dir = kroma_module_sdk::testing::temp_dir("ad-cached-path");
    let cache = Cache::new(dir.path());
    let file = crate::files::Planned {
        rel: "a.mkv".into(),
        size: 4,
        link: "l".into(),
    };
    let path = dir.path().join("cache/7/a.mkv");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();

    std::fs::write(&path, "abc").unwrap();
    assert_eq!(cache.cached_path("7", &file), None);
    std::fs::write(&path, "abcd").unwrap();
    assert_eq!(cache.cached_path("7", &file), Some(path));
}
