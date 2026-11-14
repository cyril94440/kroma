use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use kroma_module_sdk::primitives::remote_file;
use serde_json::{json, Value};

use crate::fake_alldebrid::{Fake, Script};
use crate::it_engine::engine;
use crate::{AllDebrid, TorrentState, TorrentStatus};

pub(crate) fn node(name: &str, size: usize) -> Value {
    json!({ "n": name, "s": size, "l": format!("https://alldebrid.com/f/{name}") })
}

pub(crate) fn pack() -> Script {
    let tree = json!([{ "n": "Show.S01", "e": [
        node("ep1.mkv", 5000),
        node("ep1.en.srt", 20),
        node("ep1.nfo", 10),
        node("poster.jpg", 10),
        node("ep1.fr.sub", 10),
        { "n": "Subs", "e": [node("huge.srt", 6 * 1024 * 1024)] },
    ] }]);
    Script::new(
        tree,
        vec![
            ("ep1.mkv", vec![7; 5000]),
            (
                "ep1.en.srt",
                b"1\n00:00:01,000 --> 00:00:02,000\nhi\n".to_vec()[..20].to_vec(),
            ),
        ],
        true,
    )
}

pub(crate) async fn on_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work).await.unwrap()
}

pub(crate) async fn poll(alldebrid: &Arc<AllDebrid>, id: &'static str) -> TorrentStatus {
    for _ in 0..500 {
        let engine = alldebrid.clone();
        let seen = on_thread(move || engine.status(id).unwrap().unwrap()).await;
        if seen.state != TorrentState::Downloading || seen.progress >= 1.0 {
            return seen;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the status never settled");
}

fn setup(tag: &str, fake: &Fake) -> (kroma_module_sdk::testing::TempDir, Arc<AllDebrid>) {
    let dir = kroma_module_sdk::testing::temp_dir(tag);
    let alldebrid = Arc::new(engine(fake, dir.path()));
    (dir, alldebrid)
}

fn names(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = item.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    found.sort();
    found
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_ready_magnet_becomes_stubs_and_small_subtitles_and_only_then_completed() {
    let fake = pack().start();
    let (dir, alldebrid) = setup("ad-ready", &fake);

    let done = poll(&alldebrid, "7").await;

    let folder = dir.path().join("downloads/7");
    assert_eq!(done.state, TorrentState::Completed);
    assert_eq!(done.progress, 1.0);
    assert_eq!(done.save_path.as_deref(), folder.to_str());
    assert_eq!(
        done.files,
        ["Show.S01/ep1.mkv.kremote", "Show.S01/ep1.en.srt"]
    );
    assert_eq!(
        names(&folder),
        ["Show.S01/ep1.en.srt", "Show.S01/ep1.mkv.kremote"]
    );
    let stub = remote_file::read(&folder.join("Show.S01/ep1.mkv.kremote")).unwrap();
    assert_eq!(stub.source, "alldebrid");
    assert_eq!(stub.reference, "7:Show.S01/ep1.mkv");
    assert_eq!(stub.name, "ep1.mkv");
    assert_eq!(stub.size, 5000);
    assert_eq!(
        std::fs::read(folder.join("Show.S01/ep1.en.srt"))
            .unwrap()
            .len(),
        20
    );
    let unlocked: Vec<String> = fake
        .calls_to("/v4/link/unlock")
        .iter()
        .map(|c| c.form()["link"].clone())
        .collect();
    assert_eq!(
        unlocked,
        ["https://alldebrid.com/f/ep1.en.srt"],
        "only the sidecar was downloaded"
    );
    assert!(
        fake.calls_to("/dl/ep1.mkv").is_empty(),
        "no video byte was fetched"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn completed_is_never_reported_while_a_sidecar_is_still_on_its_way() {
    let fake = pack().start();
    let (_dir, alldebrid) = setup("ad-ready-wait", &fake);

    let first = on_thread({
        let engine = alldebrid.clone();
        move || engine.status("7").unwrap().unwrap()
    })
    .await;

    if first.state == TorrentState::Completed {
        assert_eq!(first.files.len(), 2);
    } else {
        assert_eq!(
            (first.state, first.progress),
            (TorrentState::Downloading, 0.99)
        );
        assert!(first.files.is_empty() && first.save_path.is_none());
    }
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_magnet_with_no_video_is_an_error_not_an_empty_completion() {
    let script = Script::new(json!([node("readme.nfo", 4)]), vec![], true);
    let fake = script.start();
    let (_dir, alldebrid) = setup("ad-ready-novideo", &fake);

    let seen = on_thread(move || alldebrid.status("7").unwrap().unwrap()).await;

    assert_eq!(seen.state, TorrentState::Error);
    assert!(seen.error.unwrap().contains("no video"));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
async fn a_failing_subtitle_does_not_hold_the_title_back() {
    let tree = json!([node("a.mkv", 10), node("a.srt", 10)]);
    let fake = Script::new(tree, vec![("a.mkv", vec![1; 10])], true).start();
    let (_dir, alldebrid) = setup("ad-ready-badsub", &fake);

    let done = poll(&alldebrid, "7").await;

    assert_eq!(done.state, TorrentState::Completed);
    assert_eq!(done.files, ["a.mkv.kremote"]);
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn removing_a_magnet_that_has_not_finished_cancels_it_on_alldebrid() {
    let script = pack();
    script.status_code.store(1, Ordering::SeqCst);
    let fake = script.start();
    let dir = kroma_module_sdk::testing::temp_dir("ad-remove-early");

    crate::owned::Owned::new(dir.path()).claim("7");

    engine(&fake, dir.path()).remove("7", true, false).unwrap();

    assert_eq!(fake.calls_to("/v4/magnet/delete").len(), 1);
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn a_finished_magnet_stays_on_alldebrid_and_only_its_local_stubs_go() {
    let fake = pack().start();
    let dir = kroma_module_sdk::testing::temp_dir("ad-remove-ready");
    let stub = dir.path().join("downloads/7/Show.S01/ep1.mkv.kremote");
    std::fs::create_dir_all(stub.parent().unwrap()).unwrap();
    std::fs::write(&stub, "{}").unwrap();
    let cached = dir.path().join("cache/7/a.mkv");
    std::fs::create_dir_all(cached.parent().unwrap()).unwrap();
    std::fs::write(&cached, "x").unwrap();
    let alldebrid = engine(&fake, dir.path());

    alldebrid.remove("7", false, false).unwrap();
    assert!(stub.exists(), "keeping the data keeps the stubs");
    alldebrid.remove("7", true, false).unwrap();

    assert!(!dir.path().join("downloads/7").exists());
    assert!(cached.exists(), "the purge job owns the cache");
    assert!(fake.calls_to("/v4/magnet/delete").is_empty());
}

#[test]
#[cfg_attr(
    target_family = "wasm",
    ignore = "binds a test server, and a component holds no socket"
)]
fn removing_a_magnet_alldebrid_forgot_succeeds() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => crate::fake_alldebrid::Resp::failure("MAGNET_INVALID_ID", "gone"),
        _ => crate::fake_alldebrid::Resp::failure("NOPE", "x"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-remove-gone");

    assert!(engine(&fake, dir.path()).remove("7", true, false).is_ok());
}
