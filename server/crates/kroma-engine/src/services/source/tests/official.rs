use super::*;

fn contribution(module: &str, base: &str) -> Contribution {
    Contribution {
        module_id: module.into(),
        instance: Some("debrid".into()),
        base_url: base.into(),
        token: "tok".into(),
    }
}

fn sources_over(contributions: Vec<Contribution>, official: &[&str]) -> Arc<Sources> {
    let official: Vec<String> = official.iter().map(ToString::to_string).collect();
    Sources::new(
        Arc::new(move |_| contributions.clone()),
        Arc::new(move || official.clone()),
        Arc::new(|_, _| true),
        Arc::new(|_| Vec::new()),
    )
}

fn stub_in(dir: &kroma_testing::TempDir) -> String {
    let path = dir.path().join("a.kremote");
    remote_file::write(
        &path,
        &RemoteFile { source: "debrid".into(), reference: "r".into(), name: "a.mkv".into(), size: 1 },
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn a_module_the_official_registry_did_not_ship_is_never_asked_for_a_source() {
    let intruder = RecordingServer::replying(200, &url_reply("https://1.1.1.1/a").to_string());
    let sources = sources_over(vec![contribution("tv.evil.capture", intruder.base())], &[]);
    let dir = kroma_testing::temp_dir("sources-unofficial");

    let error = sources.find(&stub_in(&dir), true).unwrap_err();

    assert_eq!(error, SourceError::NoModule);
    assert!(intruder.requests().is_empty());
}

#[test]
fn an_unofficial_module_does_not_shadow_the_official_one_with_the_same_instance_id() {
    let intruder = RecordingServer::replying(200, &url_reply("https://6.6.6.6/steal").to_string());
    let genuine = RecordingServer::replying(200, &url_reply("https://1.1.1.1/a").to_string());
    let sources = sources_over(
        vec![contribution("tv.evil.capture", intruder.base()), contribution("tv.example.debrid", genuine.base())],
        &["tv.example.debrid"],
    );
    let dir = kroma_testing::temp_dir("sources-shadow");

    let found = sources.find(&stub_in(&dir), false).unwrap();

    assert!(matches!(found.place, Place::Upstream(url) if url.host() == "1.1.1.1"));
    assert!(intruder.requests().is_empty());
}

#[test]
fn two_official_modules_with_one_instance_id_are_both_refused() {
    let one = RecordingServer::replying(200, &url_reply("https://1.1.1.1/a").to_string());
    let two = RecordingServer::replying(200, &url_reply("https://1.1.1.1/b").to_string());
    let sources = sources_over(
        vec![contribution("tv.a.debrid", one.base()), contribution("tv.b.debrid", two.base())],
        &["tv.a.debrid", "tv.b.debrid"],
    );
    let dir = kroma_testing::temp_dir("sources-duplicate");

    let error = sources.find(&stub_in(&dir), false).unwrap_err();

    assert_eq!(error, SourceError::Refused(Refusal::DuplicateSource));
    assert!(one.requests().is_empty() && two.requests().is_empty());
}
