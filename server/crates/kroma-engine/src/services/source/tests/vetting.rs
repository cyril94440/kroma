use super::*;

#[test]
fn a_url_that_is_not_https_is_refused() {
    let h = harness(url_reply("http://1.1.1.1/a.mkv"));

    assert_eq!(
        h.refusal(),
        Refusal::Url(kroma_primitives::input::RemoteUrlError::NotHttps)
    );
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
    let h = harness_with(|_| url_reply("https://1.1.1.1/a.mkv"), false);

    assert_eq!(h.refusal(), Refusal::HostNotDeclared);
}

#[test]
fn a_url_on_a_host_the_source_declared_is_read() {
    let h = harness_with(|_| url_reply("https://1.1.1.1/a.mkv"), true);

    assert!(h.sources.find(&h.stub(), false).is_ok());
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
        true,
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
        true,
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
    let h = harness_with(
        |held| path_reply(&held_file(held, "Movie.mkv", SIZE - 1)),
        true,
    );

    assert_eq!(h.refusal(), Refusal::NotTheFileStubbed);
}

#[test]
fn a_path_that_is_a_folder_is_refused() {
    let h = harness_with(
        |held| {
            std::fs::create_dir_all(held.join("Movie.mkv")).unwrap();
            path_reply(&held.join("Movie.mkv"))
        },
        true,
    );

    assert_eq!(h.refusal(), Refusal::NotTheFileStubbed);
}

#[test]
fn an_answer_with_neither_a_path_nor_a_url_is_refused() {
    let h = harness(json!({ "Ok": { "path": null, "url": "", "size": SIZE } }));

    assert_eq!(h.refusal(), Refusal::NoPathNoUrl);
}

fn vetted_copy(h: &Harness) -> LocalCopy {
    let Place::Copy(copy) = h.sources.find(&h.stub(), false).unwrap().place else {
        panic!("a path answer is a copy");
    };
    copy
}

#[test]
fn a_vetted_copy_opens_as_the_file_that_was_checked() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let copy = vetted_copy(&h);

    assert_eq!(copy.open().unwrap().metadata().unwrap().len(), SIZE);
}

#[test]
fn a_vetted_copy_swapped_for_another_file_of_the_same_size_is_not_opened() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let copy = vetted_copy(&h);

    let path = h.dir.path().join("held/Movie.mkv");
    let other = h.dir.path().join("held/other.bin");
    std::fs::write(&other, vec![2u8; SIZE as usize]).unwrap();
    std::fs::rename(&other, &path).unwrap();

    assert!(copy.open().is_err());
}

#[cfg(unix)]
#[test]
fn a_vetted_copy_replaced_by_a_link_is_not_followed() {
    let h = harness_with(|held| path_reply(&held_file(held, "Movie.mkv", SIZE)), true);
    let copy = vetted_copy(&h);
    let secret = held_file(h.dir.path(), "secret.bin", SIZE);

    let path = h.dir.path().join("held/Movie.mkv");
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(&secret, &path).unwrap();

    assert!(copy.open().is_err());
}
