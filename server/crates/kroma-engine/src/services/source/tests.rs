use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use kroma_module_host::Contribution;
use kroma_module_manifest::Permissions;
use kroma_primitives::remote_file::{self, RemoteFile};

use crate::test_support::recording_server::RecordingServer;

use super::*;

const SIZE: u64 = 12;

struct Harness {
    dir: kroma_testing::TempDir,
    server: RecordingServer,
    sources: Sources,
}

fn permissions(value: Value) -> Option<Permissions> {
    Some(serde_json::from_value(value).unwrap())
}

fn harness_with(reply: impl FnOnce(&Path) -> Value, declared: Option<Permissions>) -> Harness {
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
        Arc::new(move |_| declared.clone()),
        Arc::new(move |_| vec![held.clone()]),
    );
    Harness { dir, server, sources }
}

fn harness(reply: Value) -> Harness {
    harness_with(|_| reply, None)
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
        match self.sources.resolve(&self.stub(), false) {
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
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), None);

    let resolved = h.sources.resolve(&h.stub(), false).unwrap();

    let copy = h.dir.path().join("held/Movie.mkv").canonicalize().unwrap();
    assert_eq!(resolved.input, MediaInput::Local(copy));
    assert_eq!(resolved.container().as_deref(), Some("mkv"));
}

#[test]
fn a_stub_answered_with_a_url_reads_over_https() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv?token=abc"));

    let resolved = h.sources.resolve(&h.stub(), false).unwrap();

    let MediaInput::Remote(url) = resolved.input else {
        panic!("a URL answer is a remote input");
    };
    assert_eq!(url.host(), "1.1.1.1");
    assert_eq!(url.expose(), "https://1.1.1.1/dl/a.mkv?token=abc");
}

#[test]
fn a_url_that_is_not_https_is_refused() {
    let h = harness(url_reply("http://1.1.1.1/a.mkv"));

    assert_eq!(h.refusal(), Refusal::Url(kroma_primitives::input::RemoteUrlError::NotHttps));
}

#[test]
fn a_url_naming_this_machine_or_its_network_is_refused() {
    for url in [
        "https://localhost/a.mkv",
        "https://127.0.0.1/a.mkv",
        "https://10.1.2.3/a.mkv",
        "https://192.168.0.2:8443/a.mkv",
        "https://169.254.169.254/latest",
        "https://[::1]/a.mkv",
        "https://nas/a.mkv",
        "https://user:pw@1.1.1.1/a.mkv",
    ] {
        let h = harness(url_reply(url));

        assert!(matches!(h.refusal(), Refusal::Url(_)), "{url}");
    }
}

#[test]
fn a_url_the_source_never_declared_it_connects_to_is_refused() {
    let declared = permissions(json!({ "net": { "connect": [
        { "purpose": "debrid-transfer", "to": ["*.debrid.example"], "ports": [443] }
    ] } }));
    let h = harness_with(|_| url_reply("https://1.1.1.1/a.mkv"), declared);

    assert_eq!(h.refusal(), Refusal::HostNotDeclared);
}

#[test]
fn a_url_on_a_host_the_source_declared_is_read() {
    let declared = permissions(json!({ "net": { "connect": [
        { "purpose": "debrid-transfer", "to": ["public"], "ports": [443] }
    ] } }));
    let h = harness_with(|_| url_reply("https://1.1.1.1/a.mkv"), declared);

    assert!(h.sources.resolve(&h.stub(), false).is_ok());
}

#[test]
fn a_name_that_resolves_to_this_machine_is_not_public() {
    let loopback = kroma_http::address::WebTarget { https: true, host: "localhost".into(), port: 443 };

    assert!(!vet::resolves_public(&loopback));
}

#[test]
fn a_path_outside_the_sources_folders_is_refused() {
    let outside = kroma_testing::temp_dir("sources-outside");
    let elsewhere = held_file(outside.path(), "Movie.mkv", SIZE);
    let h = harness(path_reply(&elsewhere));

    assert_eq!(h.refusal(), Refusal::PathOutsideItsFolders);
}

#[test]
fn a_path_that_climbs_out_of_the_sources_folders_is_refused() {
    let h = harness_with(
        |held| {
            held_file(held.parent().unwrap(), "secret.bin", SIZE);
            json!({ "Ok": { "path": format!("{}/../secret.bin", held.display()), "url": null, "size": SIZE } })
        },
        None,
    );

    assert_eq!(h.refusal(), Refusal::PathOutsideItsFolders);
}

#[cfg(unix)]
#[test]
fn a_link_inside_the_folders_that_leads_out_is_refused() {
    let h = harness_with(
        |held| {
            let secret = held_file(held.parent().unwrap(), "secret.bin", SIZE);
            let link = held.join("Movie.mkv");
            std::os::unix::fs::symlink(secret, &link).unwrap();
            path_reply(&link)
        },
        None,
    );

    assert_eq!(h.refusal(), Refusal::PathOutsideItsFolders);
}

#[test]
fn a_relative_path_is_refused() {
    let h = harness(json!({ "Ok": { "path": "Movie.mkv", "url": null, "size": SIZE } }));

    assert_eq!(h.refusal(), Refusal::PathOutsideItsFolders);
}

#[test]
fn a_copy_whose_length_is_not_the_stubs_is_refused() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE - 1)), None);

    assert_eq!(h.refusal(), Refusal::NotTheFileStubbed);
}

#[test]
fn a_path_that_is_a_folder_is_refused() {
    let h = harness_with(
        |held| {
            std::fs::create_dir_all(held.join("Movie.mkv")).unwrap();
            path_reply(&held.join("Movie.mkv"))
        },
        None,
    );

    assert_eq!(h.refusal(), Refusal::NotTheFileStubbed);
}

#[test]
fn an_answer_with_neither_a_path_nor_a_url_is_refused() {
    let h = harness(json!({ "Ok": { "path": null, "url": "", "size": SIZE } }));

    assert_eq!(h.refusal(), Refusal::NoPathNoUrl);
}

#[test]
fn a_source_that_answers_an_error_is_gone_and_a_retry_may_work() {
    let h = harness(json!({ "Err": "the magnet no longer has Movie.mkv" }));

    let error = h.sources.resolve(&h.stub(), false).unwrap_err();

    assert_eq!(error, SourceError::Gone("the magnet no longer has Movie.mkv".into()));
    assert!(error.is_transient());
}

#[test]
fn a_stub_naming_a_source_nobody_answers_for_has_no_module() {
    let h = harness(json!({ "Ok": {} }));
    let path = h.dir.path().join("Other.kremote");
    let stub = RemoteFile { source: "someone-else".into(), reference: "r".into(), name: "a.mkv".into(), size: 1 };
    remote_file::write(&path, &stub).unwrap();

    let error = h.sources.resolve(&path.to_string_lossy(), false).unwrap_err();

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
        Arc::new(|_| None),
        Arc::new(|_| Vec::new()),
    );
    let dir = kroma_testing::temp_dir("sources-down");
    let path = dir.path().join("a.kremote");
    remote_file::write(&path, &RemoteFile { source: "debrid".into(), reference: "r".into(), name: "a.mkv".into(), size: 1 }).unwrap();

    let error = sources.resolve(&path.to_string_lossy(), false).unwrap_err();

    assert_eq!(error, SourceError::Unreachable);
}

#[test]
fn a_stub_that_cannot_be_read_is_not_a_title() {
    let h = harness(json!({ "Ok": {} }));
    let path = h.dir.path().join("Broken.kremote");
    std::fs::write(&path, "not json").unwrap();

    let error = h.sources.resolve(&path.to_string_lossy(), false).unwrap_err();

    assert!(matches!(error, SourceError::BadStub(_)));
    assert!(!error.is_transient());
}

#[test]
fn what_is_logged_about_a_refusal_never_holds_the_url() {
    let secret = "https://127.0.0.1/bearer/abc123?sig=hunter2";
    let h = harness(url_reply(secret));

    let said = format!("{:?} / {}", h.sources.resolve(&h.stub(), false), h.sources.resolve(&h.stub(), false).unwrap_err());

    assert!(!said.contains("hunter2") && !said.contains("abc123"), "{said}");
}

#[test]
fn local_is_the_file_itself_when_it_is_not_a_stub() {
    let h = harness(json!({ "Ok": {} }));

    assert_eq!(h.sources.local("/media/a.mkv").unwrap(), Some("/media/a.mkv".into()));
}

#[test]
fn local_is_a_stubs_complete_copy_and_none_while_it_is_only_a_url() {
    let cached = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), None);
    let streamed = harness(url_reply("https://1.1.1.1/a.mkv"));

    let copy = cached.sources.local(&cached.stub()).unwrap();
    let none = streamed.sources.local(&streamed.stub()).unwrap();

    assert_eq!(copy, Some(cached.dir.path().join("held/Movie.mkv").canonicalize().unwrap()));
    assert_eq!(none, None);
}

#[test]
fn local_asks_without_warming_and_remembers_a_stub_with_no_copy() {
    let h = harness(url_reply("https://1.1.1.1/a.mkv"));
    let stub = h.stub();

    h.sources.local(&stub).unwrap();
    h.sources.local(&stub).unwrap();

    let asked = h.server.requests();
    assert_eq!(asked.len(), 1, "the second pass over the library asks nobody");
    let body: Value = serde_json::from_str(&asked[0].body).unwrap();
    assert_eq!(body["warm"], false);
}

#[test]
fn local_does_not_trust_a_remembered_copy_that_has_gone() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), None);
    let stub = h.stub();
    assert!(h.sources.local(&stub).unwrap().is_some());

    std::fs::remove_file(h.dir.path().join("held/Movie.mkv")).unwrap();

    assert!(h.sources.local(&stub).is_err(), "the source is asked again and refuses a missing file");
}
