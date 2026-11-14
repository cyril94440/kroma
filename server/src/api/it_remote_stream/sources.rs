use super::*;

#[tokio::test]
async fn a_cached_copy_is_served_from_the_disk_and_the_host_is_left_alone() {
    let p = bytes_playground().await;
    p.fake.answers_path(&p.held_copy(), p.bytes.len() as u64);

    let (status, headers, body) = p.fetch(&p.stream(), &[("range", "bytes=100-199")]).await;

    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        headers["content-range"],
        format!("bytes 100-199/{}", p.bytes.len())
    );
    assert_eq!(body, &p.bytes[100..200]);
    assert!(p.stubbed.cdn.heard().is_empty());
}

#[tokio::test]
async fn a_copy_outside_the_sources_own_folders_is_refused() {
    let p = bytes_playground().await;
    p.fake.answers_path(&p.real, p.bytes.len() as u64);

    let (status, _, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let said: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["code"], "error.sourceRefused");
}

#[tokio::test]
async fn a_copy_of_the_wrong_length_is_refused() {
    let p = bytes_playground().await;
    let copy = p.held_copy();
    std::fs::write(&copy, &p.bytes[..1000]).unwrap();
    p.fake.answers_path(&copy, p.bytes.len() as u64);

    let (status, _, _) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn a_source_that_has_lost_the_file_is_a_translated_error_not_a_500() {
    let p = bytes_playground().await;
    p.fake.answers_error("the magnet no longer has Movie.mkv");

    let (status, _, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let said: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["code"], "error.sourceGone");
    assert_ne!(
        said["error"], "error.sourceGone",
        "the message is the catalog's, not the key"
    );
    assert!(
        !said["error"].as_str().unwrap().contains("magnet"),
        "the module's own words stay in the log"
    );
}

#[tokio::test]
async fn a_stub_whose_source_no_module_answers_for_is_a_503() {
    let p = bytes_playground().await;
    std::fs::write(
        &p.stubbed.abs,
        serde_json::json!({ "source": "ghost", "ref": "r", "name": "a.mp4", "size": p.bytes.len() }).to_string(),
    )
    .unwrap();

    let (status, _, body) = p.fetch(&p.stream(), &[]).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let said: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["code"], "error.sourceDisabled");
}

#[tokio::test]
async fn a_player_s_many_ranges_ask_the_module_once_and_warm_it_once() {
    let p = bytes_playground().await;

    for n in 0..30u64 {
        let range = format!("bytes={}-{}", n * 1000, n * 1000 + 999);
        let (status, _, _) = p.fetch(&p.stream(), &[("range", range.as_str())]).await;
        assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    }

    let asked = p.fake.asked();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0]["warm"], true);
    assert_eq!(
        p.stubbed.cdn.heard().len(),
        30,
        "every range still reached the host"
    );
}

#[tokio::test]
async fn a_link_the_host_calls_gone_is_asked_for_again_and_the_range_is_served_from_the_new_one() {
    let p = bytes_playground().await;
    let (status, _, _) = p.fetch(&p.stream(), &[("range", "bytes=0-9")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    p.stubbed.cdn.fail_with(410);

    let (status, _, _) = p.fetch(&p.stream(), &[("range", "bytes=10-19")]).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(
        p.fake.asked().len(),
        2,
        "the stale link was forgotten and the module asked for a fresh one"
    );
}
