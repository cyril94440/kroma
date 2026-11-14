use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use kroma_module_host::Contribution;
use kroma_primitives::input::RemoteUrl;
use kroma_primitives::remote_file::{self, RemoteFile};

use crate::test_support::recording_server::RecordingServer;

use super::*;

const SIZE: u64 = 12;

struct Harness {
    dir: kroma_testing::TempDir,
    server: RecordingServer,
    sources: Arc<Sources>,
}

fn harness_with(reply: impl FnOnce(&Path) -> Value, declares: bool) -> Harness {
    harness_for(reply, vec!["tv.example.debrid".to_string()], declares)
}

fn harness_for(
    reply: impl FnOnce(&Path) -> Value,
    official: Vec<String>,
    declares: bool,
) -> Harness {
    let dir = kroma_testing::temp_dir("sources");
    let held = dir.path().join("held");
    std::fs::create_dir_all(&held).unwrap();
    let server = RecordingServer::replying(200, &reply(&held).to_string());
    let base = server.base().to_string();
    let contributions: Contributions = Arc::new(move |point| {
        if point != POINT {
            return Vec::new();
        }
        vec![Contribution {
            module_id: "tv.example.debrid".into(),
            instance: Some("debrid".into()),
            base_url: base.clone(),
            token: "tok".into(),
        }]
    });
    let sources = Sources::new(
        contributions,
        Arc::new(move || official.clone()),
        Arc::new(move |_, _| declares),
        Arc::new(move |_| vec![held.clone()]),
    );
    Harness {
        dir,
        server,
        sources,
    }
}

fn harness(reply: Value) -> Harness {
    harness_with(|_| reply, true)
}

fn held_file(held: &Path, name: &str, bytes: u64) -> std::path::PathBuf {
    let path = held.join(name);
    std::fs::write(&path, vec![1u8; bytes as usize]).unwrap();
    path
}

fn path_reply(path: &Path) -> Value {
    json!({ "Ok": { "path": path.to_string_lossy(), "url": null, "size": SIZE } })
}

fn url_reply(url: &str) -> Value {
    json!({ "Ok": { "path": null, "url": url, "size": SIZE } })
}

impl Harness {
    fn stub(&self) -> String {
        let path = self.dir.path().join("Movie (2020).kremote");
        let stub = RemoteFile {
            source: "debrid".into(),
            reference: "7:Movie.mkv".into(),
            name: "Movie.MKV".into(),
            size: SIZE,
        };
        remote_file::write(&path, &stub).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn refusal(&self) -> Refusal {
        match self.sources.find(&self.stub(), false) {
            Err(SourceError::Refused(rule)) => rule,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}

#[test]
fn a_file_that_is_not_a_stub_is_its_own_input_and_asks_nobody() {
    let h = harness(json!({ "Err": "never asked" }));

    let resolved = h.sources.resolve("/media/Movie.mkv", true).unwrap();

    assert_eq!(resolved.input, MediaInput::Local("/media/Movie.mkv".into()));
    assert_eq!(resolved.stub, None);
    assert!(h.server.requests().is_empty());
}

#[test]
fn a_stub_asks_its_own_source_for_its_ref_and_says_whether_to_warm() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv?token=abc"));

    h.sources.resolve(&h.stub(), true).unwrap();

    let asked = h.server.requests();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].target, "/_port/source/locate");
    assert_eq!(asked[0].header("authorization"), Some("Bearer tok"));
    let body: Value = serde_json::from_str(&asked[0].body).unwrap();
    assert_eq!(body, json!({ "ref": "7:Movie.mkv", "warm": true }));
}

#[test]
fn a_stub_answered_with_a_complete_copy_in_the_sources_folders_reads_the_copy() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);

    let resolved = h.sources.resolve(&h.stub(), false).unwrap();

    let copy = h.dir.path().join("held/Movie.mkv").canonicalize().unwrap();
    assert_eq!(resolved.input, MediaInput::Local(copy));
    assert_eq!(resolved.container().as_deref(), Some("mkv"));
}

#[test]
fn a_stub_answered_with_a_url_reads_over_https() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv?token=abc"));

    let found = h.sources.find(&h.stub(), false).unwrap();

    let Place::Upstream(url) = found.place else {
        panic!("a URL answer is an upstream");
    };
    assert_eq!(url.host(), "1.1.1.1");
    assert_eq!(url.expose(), "https://1.1.1.1/dl/a.mkv?token=abc");
}

#[test]
fn a_source_that_answers_an_error_is_gone_and_a_retry_may_work() {
    let h = harness(json!({ "Err": "the magnet no longer has Movie.mkv" }));

    let error = h.sources.resolve(&h.stub(), false).unwrap_err();

    assert_eq!(
        error,
        SourceError::Gone("the magnet no longer has Movie.mkv".into())
    );
    assert!(error.is_transient());
}

#[test]
fn a_stub_naming_a_source_nobody_answers_for_has_no_module() {
    let h = harness(json!({ "Ok": {} }));
    let path = h.dir.path().join("Other.kremote");
    let stub = RemoteFile {
        source: "someone-else".into(),
        reference: "r".into(),
        name: "a.mkv".into(),
        size: 1,
    };
    remote_file::write(&path, &stub).unwrap();

    let error = h
        .sources
        .resolve(&path.to_string_lossy(), false)
        .unwrap_err();

    assert_eq!(error, SourceError::NoModule);
    assert!(h.server.requests().is_empty());
}

#[test]
fn a_module_that_is_not_answering_is_unreachable() {
    let sources = Sources::new(
        Arc::new(|_| {
            vec![Contribution {
                module_id: "m".into(),
                instance: Some("debrid".into()),
                base_url: "http://127.0.0.1:1".into(),
                token: "t".into(),
            }]
        }),
        Arc::new(|| vec!["m".to_string()]),
        Arc::new(|_, _| true),
        Arc::new(|_| Vec::new()),
    );
    let dir = kroma_testing::temp_dir("sources-down");
    let path = dir.path().join("a.kremote");
    remote_file::write(
        &path,
        &RemoteFile {
            source: "debrid".into(),
            reference: "r".into(),
            name: "a.mkv".into(),
            size: 1,
        },
    )
    .unwrap();

    let error = sources.resolve(&path.to_string_lossy(), false).unwrap_err();

    assert_eq!(error, SourceError::Unreachable);
}

#[test]
fn a_stub_that_cannot_be_read_is_not_a_title() {
    let h = harness(json!({ "Ok": {} }));
    let path = h.dir.path().join("Broken.kremote");
    std::fs::write(&path, "not json").unwrap();

    let error = h
        .sources
        .resolve(&path.to_string_lossy(), false)
        .unwrap_err();

    assert!(matches!(error, SourceError::BadStub(_)));
    assert!(!error.is_transient());
}

#[test]
fn what_is_logged_about_a_refusal_never_holds_the_url() {
    let secret = "https://127.0.0.1/bearer/abc123?sig=hunter2";
    let h = harness(url_reply(secret));

    let said = format!(
        "{:?} / {}",
        h.sources.resolve(&h.stub(), false),
        h.sources.resolve(&h.stub(), false).unwrap_err()
    );

    assert!(
        !said.contains("hunter2") && !said.contains("abc123"),
        "{said}"
    );
}

#[test]
fn local_is_the_file_itself_when_it_is_not_a_stub() {
    let h = harness(json!({ "Ok": {} }));

    assert_eq!(
        h.sources.local("/media/a.mkv").unwrap(),
        Some("/media/a.mkv".into())
    );
}

#[test]
fn local_is_a_stubs_complete_copy_and_none_while_it_is_only_a_url() {
    let cached = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let streamed = harness(url_reply("https://1.1.1.1/a.mkv"));

    let copy = cached.sources.local(&cached.stub()).unwrap();
    let none = streamed.sources.local(&streamed.stub()).unwrap();

    assert_eq!(
        copy,
        Some(
            cached
                .dir
                .path()
                .join("held/Movie.mkv")
                .canonicalize()
                .unwrap()
        )
    );
    assert_eq!(none, None);
}

#[test]
fn local_asks_without_warming_and_remembers_a_stub_with_no_copy() {
    let h = harness(url_reply("https://1.1.1.1/a.mkv"));
    let stub = h.stub();

    h.sources.local(&stub).unwrap();
    h.sources.local(&stub).unwrap();

    let asked = h.server.requests();
    assert_eq!(
        asked.len(),
        1,
        "the second pass over the library asks nobody"
    );
    let body: Value = serde_json::from_str(&asked[0].body).unwrap();
    assert_eq!(body["warm"], false);
}

#[test]
fn local_does_not_trust_a_remembered_copy_that_has_gone() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let stub = h.stub();
    assert!(h.sources.local(&stub).unwrap().is_some());

    std::fs::remove_file(h.dir.path().join("held/Movie.mkv")).unwrap();

    assert!(
        h.sources.local(&stub).is_err(),
        "the source is asked again and refuses a missing file"
    );
}

mod granted;
mod memo;
mod official;
mod upstream;
mod vetting;

fn public_name(server: &kroma_testing::RangedServer) -> String {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let host = format!(
        "cdn-{}.example.com",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    );
    kroma_http::test_hosts::map_host(
        &host,
        "93.184.216.34".parse().unwrap(),
        std::net::SocketAddr::from(([127, 0, 0, 1], server.port())),
    );
    host
}

fn upstream(host: &str) -> RemoteUrl {
    RemoteUrl::parse(&format!("https://{host}/dl/a.mkv?token=hunter2")).unwrap()
}

async fn read_all(mut body: Upstream) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(&mut body, &mut bytes).await?;
    Ok(bytes)
}
