use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use crate::cache::{Availability, Limits};
use crate::fake_alldebrid::{host_of, Fake, Req, Resp};
use crate::it_engine::engine;
use crate::it_ready::node;
use crate::AllDebrid;

const SIZE: usize = 200_000;

fn body() -> Vec<u8> {
    (0..SIZE).map(|i| (i % 251) as u8).collect()
}

fn account(dl: impl Fn(&Req, &[u8]) -> Resp + Send + Sync + 'static) -> Fake {
    let whole = body();
    Fake::start(move |req| match req.path.as_str() {
        "/v4/magnet/files" => Resp::data(json!({ "magnets": [{ "id": "7", "files": [
            node("a.mkv", SIZE), node("b.mkv", SIZE)
        ] }] })),
        "/v4/link/unlock" => {
            let name = req.form()["link"].rsplit('/').next().unwrap().to_string();
            Resp::data(json!({ "link": format!("http://{}/dl/{name}", host_of(req)), "filesize": SIZE }))
        }
        path if path.starts_with("/dl/") => dl(req, &whole),
        _ => Resp::failure("NOPE", "unexpected call"),
    })
}

fn limits(max_bytes: u64) -> Limits {
    Limits {
        max_bytes: Some(max_bytes),
        max_age: Duration::ZERO,
    }
}

fn setup(tag: &str, fake: &Fake) -> (kroma_module_sdk::testing::TempDir, Arc<AllDebrid>) {
    let dir = kroma_module_sdk::testing::temp_dir(tag);
    let ad = Arc::new(engine(fake, dir.path()));
    (dir, ad)
}

async fn on_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work).await.unwrap()
}

async fn finished(ad: &Arc<AllDebrid>, rel: &'static str, limits: Limits) -> Availability {
    for _ in 0..800 {
        let ad = ad.clone();
        let seen = on_thread(move || ad.cache_file("7", rel, limits).unwrap()).await;
        if !matches!(seen, Availability::Fetching { .. }) {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the fetch did not finish");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_file_larger_than_the_whole_budget_is_declined_and_nothing_is_fetched() {
    let fake = account(|req, whole| Resp::bytes(req, whole, true));
    let (dir, ad) = setup("ad-limit-toobig", &fake);

    let seen = finished(&ad, "a.mkv", limits(SIZE as u64 - 1)).await;

    assert!(matches!(seen, Availability::Declined(_)), "{seen:?}");
    assert!(fake.calls_to("/dl/a.mkv").is_empty());
    assert!(!dir.path().join("cache").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_file_that_would_overrun_the_budget_is_declined_while_the_first_runs() {
    let fake = account(|req, whole| {
        std::thread::sleep(Duration::from_millis(150));
        Resp::bytes(req, whole, true)
    });
    let (dir, ad) = setup("ad-limit-second", &fake);
    let lim = limits(SIZE as u64 + 50_000);

    let first = {
        let ad = ad.clone();
        on_thread(move || ad.cache_file("7", "a.mkv", lim).unwrap()).await
    };
    let second = {
        let ad = ad.clone();
        on_thread(move || ad.cache_file("7", "b.mkv", lim).unwrap()).await
    };

    assert!(matches!(first, Availability::Fetching { .. }), "{first:?}");
    assert!(matches!(second, Availability::Declined(_)), "{second:?}");
    assert!(fake.calls_to("/dl/b.mkv").is_empty());
    assert!(matches!(
        finished(&ad, "a.mkv", lim).await,
        Availability::Cached(_)
    ));
    drop(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_file_idle_past_the_grace_makes_room_for_the_next() {
    let fake = account(|req, whole| Resp::bytes(req, whole, true));
    let (dir, ad) = setup("ad-limit-evict", &fake);
    let lim = limits(SIZE as u64 + 10);
    let Availability::Cached(first) = finished(&ad, "a.mkv", lim).await else {
        panic!("a.mkv was not cached");
    };
    let long_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(&first)
        .unwrap()
        .set_modified(long_ago)
        .unwrap();

    let second = finished(&ad, "b.mkv", lim).await;

    assert!(matches!(second, Availability::Cached(_)), "{second:?}");
    assert!(!first.exists());
    drop(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_range_the_server_did_not_answer_is_refused_and_never_appended() {
    let fake = account(|req, whole| {
        let (from, to) = req.range().unwrap();
        let end = (to as usize + 1).min(whole.len());
        Resp::raw(206, whole[from as usize..end].to_vec()).header(
            "Content-Range",
            format!("bytes {}-{}/{}", from + 1, end, whole.len()),
        )
    });
    let (dir, ad) = setup("ad-limit-range", &fake);

    let seen = finished(&ad, "a.mkv", limits(1 << 30)).await;

    let Availability::Failed(reason) = seen else {
        panic!("{seen:?}")
    };
    assert!(reason.contains("range"), "{reason}");
    let part = dir.path().join("cache/7/a.mkv.part");
    assert!(!part.exists() || std::fs::metadata(part).unwrap().len() == 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn relocks_are_counted_per_stall_and_not_per_file() {
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let fake = account(move |req, whole| {
        if counted.fetch_add(1, Ordering::SeqCst).is_multiple_of(2) {
            Resp::raw(403, Vec::new())
        } else {
            Resp::bytes(req, whole, true)
        }
    });
    let (_dir, ad) = setup("ad-limit-relock", &fake);

    let seen = finished(&ad, "a.mkv", limits(1 << 30)).await;

    assert!(matches!(seen, Availability::Cached(_)), "{seen:?}");
    assert!(hits.load(Ordering::SeqCst) >= 8);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_oversized_whole_file_answer_to_a_range_is_not_accepted() {
    let fake = account(|_, whole| {
        let mut more = whole.to_vec();
        more.extend_from_slice(b"extra");
        Resp::raw(200, more)
    });
    let (dir, ad) = setup("ad-limit-oversize", &fake);

    let seen = finished(&ad, "a.mkv", limits(1 << 30)).await;

    assert!(matches!(seen, Availability::Failed(_)), "{seen:?}");
    assert!(!dir.path().join("cache/7/a.mkv").exists());
}
