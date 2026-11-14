use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::cache::{Availability, Cache};
use crate::fake_alldebrid::{host_of, scripted, Fake, Resp};
use crate::it_engine::engine;
use crate::AllDebrid;

fn node(name: &str, size: usize) -> Value {
    json!({ "n": name, "s": size, "l": format!("https://alldebrid.com/f/{name}") })
}

fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

async fn on_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work).await.unwrap()
}

async fn cached(alldebrid: &Arc<AllDebrid>, rel: &'static str) -> Availability {
    for _ in 0..500 {
        let engine = alldebrid.clone();
        let seen = on_thread(move || engine.cache_file("7", rel).unwrap()).await;
        if !matches!(seen, Availability::Fetching { .. }) {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the fetch did not finish");
}

fn leftovers(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = item.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|e| e == "part" || e == "chunk")
            {
                found.push(path.display().to_string());
            }
        }
    }
    found
}

fn setup(tag: &str, fake: &Fake) -> (kroma_module_sdk::testing::TempDir, Arc<AllDebrid>) {
    let dir = kroma_module_sdk::testing::temp_dir(tag);
    let alldebrid = Arc::new(engine(fake, dir.path()));
    (dir, alldebrid)
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_file_arrives_chunk_by_chunk_and_only_its_final_name_is_ever_complete() {
    let body = payload(200_000);
    let fake = scripted(
        json!([{ "n": "Show", "e": [node("a.mkv", body.len())] }]),
        vec![("a.mkv", body.clone())],
        true,
    );
    let (dir, alldebrid) = setup("ad-cache-chunks", &fake);

    let landed = cached(&alldebrid, "Show/a.mkv").await;

    let Availability::Cached(path) = landed else {
        panic!("{landed:?}")
    };
    assert_eq!(path, dir.path().join("cache/7/Show/a.mkv"));
    assert_eq!(std::fs::read(&path).unwrap(), body);
    assert!(leftovers(&dir.path().join("cache")).is_empty());
    assert!(
        fake.calls_to("/dl/a.mkv").len() >= 4,
        "bounded ranged requests"
    );
    assert!(fake
        .calls_to("/dl/a.mkv")
        .iter()
        .all(|c| c.range().is_some()));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_started_fetch_is_joined_not_duplicated() {
    let body = payload(300_000);
    let fake = scripted(
        json!([node("a.mkv", body.len())]),
        vec![("a.mkv", body)],
        true,
    );
    let (_dir, alldebrid) = setup("ad-cache-join", &fake);

    let engine = alldebrid.clone();
    let first = on_thread(move || engine.cache_file("7", "a.mkv").unwrap()).await;
    let engine = alldebrid.clone();
    let second = on_thread(move || engine.cache_file("7", "a.mkv").unwrap()).await;
    let landed = cached(&alldebrid, "a.mkv").await;

    assert!(
        matches!(first, Availability::Fetching { total: 300_000, .. }),
        "{first:?}"
    );
    assert!(
        matches!(
            second,
            Availability::Fetching { .. } | Availability::Cached(_)
        ),
        "{second:?}"
    );
    assert!(matches!(landed, Availability::Cached(_)));
    assert_eq!(
        fake.calls_to("/v4/link/unlock").len(),
        1,
        "one unlock for one running fetch"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_partial_file_resumes_at_the_byte_it_reached() {
    let body = payload(200_000);
    let fake = scripted(
        json!([node("a.mkv", body.len())]),
        vec![("a.mkv", body.clone())],
        true,
    );
    let (dir, alldebrid) = setup("ad-cache-resume", &fake);
    let part = dir.path().join("cache/7/a.mkv.part");
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, &body[..131_072]).unwrap();

    let landed = cached(&alldebrid, "a.mkv").await;

    let Availability::Cached(path) = landed else {
        panic!("{landed:?}")
    };
    assert_eq!(std::fs::read(path).unwrap(), body);
    let starts: Vec<u64> = fake
        .calls_to("/dl/a.mkv")
        .iter()
        .map(|c| c.range().unwrap().0)
        .collect();
    assert_eq!(starts.first(), Some(&131_072), "{starts:?}");
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_complete_file_costs_no_download_and_is_touched() {
    let body = payload(1000);
    let fake = scripted(
        json!([node("a.mkv", body.len())]),
        vec![("a.mkv", body.clone())],
        true,
    );
    let (dir, alldebrid) = setup("ad-cache-hit", &fake);
    let path = dir.path().join("cache/7/a.mkv");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &body).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(86_400 * 30);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old)
        .unwrap();

    let landed = cached(&alldebrid, "a.mkv").await;

    assert!(matches!(landed, Availability::Cached(_)));
    assert!(fake.calls_to("/dl/a.mkv").is_empty());
    let used = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert!(
        used > old + Duration::from_secs(86_400),
        "last use was refreshed"
    );
}

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
