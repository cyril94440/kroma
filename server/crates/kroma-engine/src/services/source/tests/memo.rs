use super::*;

fn asked_warm(h: &Harness) -> Vec<bool> {
    h.server
        .requests()
        .iter()
        .map(|request| {
            serde_json::from_str::<Value>(&request.body).unwrap()["warm"]
                .as_bool()
                .unwrap()
        })
        .collect()
}

#[test]
fn many_range_requests_for_one_stub_ask_the_module_once_and_warm_it_once() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();

    for _ in 0..50 {
        h.sources.find(&stub, true).unwrap();
    }

    assert_eq!(asked_warm(&h), [true]);
}

#[test]
fn a_call_that_must_warm_is_not_answered_from_one_that_did_not() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();

    h.sources.find(&stub, false).unwrap();
    h.sources.find(&stub, true).unwrap();
    h.sources.find(&stub, false).unwrap();
    h.sources.find(&stub, true).unwrap();

    assert_eq!(asked_warm(&h), [false, true]);
}

#[test]
fn a_stub_with_other_content_is_not_answered_from_the_memory_of_the_old_one() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();
    h.sources.find(&stub, false).unwrap();

    let other = RemoteFile {
        source: "debrid".into(),
        reference: "8:Other.mkv".into(),
        name: "Movie.MKV".into(),
        size: SIZE,
    };
    remote_file::write(Path::new(&stub), &other).unwrap();
    h.sources.find(&stub, false).unwrap();

    let refs: Vec<String> = h
        .server
        .requests()
        .iter()
        .map(|request| {
            serde_json::from_str::<Value>(&request.body).unwrap()["ref"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(refs, ["7:Movie.mkv", "8:Other.mkv"]);
}

#[test]
fn a_link_the_upstream_called_gone_is_forgotten_at_once() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();
    h.sources.find(&stub, false).unwrap();

    h.sources.forget(&stub);
    h.sources.find(&stub, false).unwrap();

    assert_eq!(h.server.requests().len(), 2);
}

#[test]
fn a_local_copy_that_appears_wins_over_the_link_remembered_before_it() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();
    assert!(matches!(
        h.sources.find(&stub, false).unwrap().place,
        Place::Upstream(_)
    ));

    let copy = held_file(&h.dir.path().join("held"), "Movie.mkv", SIZE);
    h.server
        .answer_from_now_on(200, &path_reply(&copy).to_string());
    h.sources.forget(&stub);

    assert!(matches!(
        h.sources.find(&stub, false).unwrap().place,
        Place::Copy(_)
    ));
    h.server.answer_from_now_on(
        200,
        &json!({ "Err": "must not be asked again" }).to_string(),
    );
    assert!(
        matches!(h.sources.find(&stub, false).unwrap().place, Place::Copy(_)),
        "the copy answers with no module call"
    );
    assert_eq!(h.server.requests().len(), 2);
}

#[test]
fn a_copy_that_vanishes_is_not_trusted_and_the_module_is_asked_again() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let stub = h.stub();
    h.sources.find(&stub, false).unwrap();

    std::fs::remove_file(h.dir.path().join("held/Movie.mkv")).unwrap();

    assert!(h.sources.find(&stub, false).is_err());
    assert_eq!(h.server.requests().len(), 2);
}

#[test]
fn a_source_that_is_down_is_asked_once_by_a_pass_over_the_library_not_once_per_stage() {
    let h = harness(json!({ "Err": "the module is starting" }));
    let stub = h.stub();

    for _ in 0..20 {
        assert!(h.sources.local(&stub).is_err());
    }

    assert_eq!(h.server.requests().len(), 1);
}

#[test]
fn a_copy_that_lands_after_a_failure_is_found_once_the_failure_is_forgotten() {
    let h = harness(json!({ "Err": "the module is starting" }));
    let stub = h.stub();
    assert!(h.sources.local(&stub).is_err());

    let copy = held_file(&h.dir.path().join("held"), "Movie.mkv", SIZE);
    h.server
        .answer_from_now_on(200, &path_reply(&copy).to_string());
    h.sources.forget(&stub);
    h.sources.find(&stub, false).unwrap();

    assert!(h.sources.local(&stub).unwrap().is_some());
}

#[test]
fn a_probe_that_cannot_read_the_network_warms_at_most_three_titles_at_once() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stubs: Vec<String> = (0..5)
        .map(|n| {
            let path = h.dir.path().join(format!("m{n}.kremote"));
            let stub = RemoteFile {
                source: "debrid".into(),
                reference: format!("{n}:a.mkv"),
                name: "a.mkv".into(),
                size: SIZE,
            };
            remote_file::write(&path, &stub).unwrap();
            path.to_string_lossy().into_owned()
        })
        .collect();

    for stub in &stubs {
        let error = h.sources.resolve_probing(stub, false).unwrap_err();
        assert_eq!(error, SourceError::Preparing);
    }

    let warmed = asked_warm(&h).into_iter().filter(|warm| *warm).count();
    assert_eq!(warmed, 3);
}

#[test]
fn a_re_read_that_cannot_read_the_network_never_asks_a_source_to_cache_a_title() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let stub = h.stub();

    let error = h.sources.resolve_quietly(&stub, false).unwrap_err();

    assert_eq!(error, SourceError::Preparing);
    assert!(asked_warm(&h).iter().all(|warm| !warm));
}

#[test]
fn a_re_read_of_a_file_on_disk_needs_no_network_at_all() {
    let h = harness(url_reply("https://1.1.1.1/dl/a.mkv"));
    let path = h.dir.path().join("plain.mkv");
    std::fs::write(&path, b"x").unwrap();

    let resolved = h
        .sources
        .resolve_quietly(&path.to_string_lossy(), false)
        .unwrap();

    assert_eq!(resolved.input.local(), Some(path.as_path()));
    assert!(h.server.requests().is_empty());
}
