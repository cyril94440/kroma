use super::*;

#[tokio::test]
async fn hls_remuxes_a_stub_from_the_url_and_serves_its_segments() {
    let p = clip_playground().await;
    probe(&p.t, &p.stubbed).await.unwrap();
    let master = format!("/api/items/{}/hls/copy/0/0/index.m3u8", p.stubbed.item);

    let (status, _, playlist) = p.fetch(&master, &[]).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&playlist)
    );
    let playlist = String::from_utf8(playlist).unwrap();
    assert!(playlist.contains("#EXT-X-TARGETDURATION"), "{playlist}");
    let base = format!("/api/items/{}/hls/copy/0/0", p.stubbed.item);
    let (status, _, init) = p.fetch(&format!("{base}/init.mp4"), &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&init[4..8], b"ftyp");
    let (status, _, segment) = p.fetch(&format!("{base}/seg_00000.m4s"), &[]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!segment.is_empty());
    assert!(p.stubbed.cdn.heard().iter().any(|r| r.range.is_some()));
}

#[tokio::test]
async fn hls_of_a_stub_whose_source_is_down_is_a_translated_502() {
    let p = clip_playground().await;
    probe(&p.t, &p.stubbed).await.unwrap();
    p.fake.answers_error("down");

    let (status, _, body) = p
        .fetch(
            &format!("/api/items/{}/hls/copy/0/0/index.m3u8", p.stubbed.item),
            &[],
        )
        .await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    let said: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(said["code"], "error.sourceGone");
}

#[tokio::test]
async fn a_stub_is_downloaded_as_a_remuxed_mp4_read_from_the_url() {
    let p = clip_playground().await;
    probe(&p.t, &p.stubbed).await.unwrap();

    let (status, _, body) = p
        .fetch(&format!("/api/items/{}/download", p.stubbed.item), &[])
        .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body[4..8], b"ftyp");
}

#[tokio::test]
async fn a_scrub_sheet_waits_for_a_local_copy_and_is_never_drawn_over_the_network() {
    let p = clip_playground().await;
    probe(&p.t, &p.stubbed).await.unwrap();
    let uri = format!("/api/items/{}/storyboard", p.stubbed.item);
    let read_so_far = p.stubbed.cdn.heard().len();

    let (status, _, _) = p.fetch(&uri, &[]).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        p.stubbed.cdn.heard().len(),
        read_so_far,
        "nothing was read for a sheet"
    );
}
