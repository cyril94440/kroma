use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::cache::{Availability, Limits};
use crate::fake_alldebrid::{scripted, Fake};
use crate::it_engine::engine;
use crate::AllDebrid;

pub(super) fn wide() -> Limits {
    Limits {
        max_bytes: Some(1 << 40),
        max_age: Duration::ZERO,
    }
}

pub(super) fn node(name: &str, size: usize) -> Value {
    json!({ "n": name, "s": size, "l": format!("https://alldebrid.com/f/{name}") })
}

pub(super) fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

pub(super) async fn on_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work).await.unwrap()
}

pub(super) async fn cached(alldebrid: &Arc<AllDebrid>, rel: &'static str) -> Availability {
    for _ in 0..500 {
        let engine = alldebrid.clone();
        let seen = on_thread(move || engine.cache_file("7", rel, wide()).unwrap()).await;
        if !matches!(seen, Availability::Fetching { .. }) {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the fetch did not finish");
}

pub(super) fn leftovers(root: &Path) -> Vec<String> {
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

pub(super) fn setup(tag: &str, fake: &Fake) -> (kroma_module_sdk::testing::TempDir, Arc<AllDebrid>) {
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
    let first = on_thread(move || engine.cache_file("7", "a.mkv", wide()).unwrap()).await;
    let engine = alldebrid.clone();
    let second = on_thread(move || engine.cache_file("7", "a.mkv", wide()).unwrap()).await;
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
