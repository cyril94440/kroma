use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use kroma_media::hls::{Asked, AudioMode, HlsEngine, StreamMode, VideoMode};
use kroma_media::probe::{probe_outcome, Probed};
use kroma_testing::RangedServer;

use crate::services::source::loopback::{authorise, Loopback, Table};
use super::*;

struct Titled {
    dir: kroma_testing::TempDir,
    cdn: RangedServer,
    harness: Harness,
    stub: String,
}

fn titled() -> Titled {
    let dir = kroma_testing::temp_dir("sources-loopback");
    let clip = kroma_testing::clip(dir.path());
    let size = std::fs::metadata(&clip).unwrap().len();
    let cdn = RangedServer::serve(&clip);
    let url = format!("https://{}/dl/clip.mp4?token=hunter2", public_name(&cdn));
    let harness = harness(json!({ "Ok": { "path": null, "url": url, "size": size } }));
    let stub = dir.path().join("Clip.kremote");
    remote_file::write(
        &stub,
        &RemoteFile { source: "debrid".into(), reference: "1:clip.mp4".into(), name: "clip.mp4".into(), size },
    )
    .unwrap();
    Titled { dir, cdn, harness, stub: stub.to_string_lossy().into_owned() }
}

impl Titled {
    fn input(&self) -> MediaInput {
        let found = self.harness.sources.find(&self.stub, false).unwrap();
        self.harness.sources.input_of(&self.stub, found, true).unwrap().input
    }
}

fn mode() -> StreamMode {
    StreamMode::new(VideoMode::Copy, AudioMode::Copy)
}

#[test]
fn ffmpeg_is_given_a_loopback_address_and_never_the_modules_url() {
    let t = titled();

    let MediaInput::Remote(relayed) = t.input() else { panic!("a URL answer is relayed") };

    let args: Vec<String> = kroma_primitives::input::media(&MediaInput::Remote(relayed))
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let said = args.join(" ");
    assert!(said.contains("http,tcp") && !said.contains("https"), "{said}");
    assert!(said.contains("http://127.0.0.1:"), "{said}");
    assert!(!said.contains("example.com") && !said.contains("hunter2"), "{said}");
}

#[test]
fn a_title_is_probed_through_the_relay_with_ranges_and_the_upstream_sees_the_policy_path() {
    let t = titled();
    let input = t.input();

    let probed = probe_outcome(&input, "mp4", true);

    let Probed::Done(result) = probed else { panic!("{probed:?}") };
    assert_eq!(result.unreadable, None);
    assert_eq!(result.video.as_ref().map(|v| v.codec.as_str()), Some("h264"));
    assert!(t.cdn.heard().iter().all(|r| r.range.is_some()), "{:?}", t.cdn.heard());
}

#[test]
fn a_probe_never_follows_an_upstream_redirect_to_a_private_target() {
    let t = titled();
    let private = RangedServer::serve(&t.dir.path().join("clip.mp4"));
    t.cdn.redirect_to(&format!("http://127.0.0.1:{}/clip.mp4", private.port()));
    let input = t.input();

    let probed = probe_outcome(&input, "mp4", true);

    assert!(matches!(probed, Probed::Network), "{probed:?}");
    assert!(private.heard().is_empty(), "the redirect target was never contacted");
}

#[tokio::test]
async fn an_hls_master_is_cut_through_the_relay_and_never_follows_a_redirect() {
    let t = titled();
    let engine = HlsEngine::new(t.dir.path().join("hls"), 2, 0);
    let input = t.input();
    let asked = |input| Asked {
        item_id: "it1",
        input,
        opener: None,
        audio: 0,
        mode: mode(),
        anchor: 0,
        source: Some((320, 180)),
        video: None,
        track: None,
    };

    let (playlist, start) = engine.master(asked(&input)).await.expect("a playlist through the relay");

    assert!(playlist.contains("#EXT-X-TARGETDURATION"));
    assert_eq!(start, 0.0);

    let private = RangedServer::serve(&t.dir.path().join("clip.mp4"));
    t.cdn.redirect_to(&format!("http://127.0.0.1:{}/clip.mp4", private.port()));
    let elsewhere = engine2(&t, 1);
    let second = elsewhere.master(asked(&input)).await;
    assert!(private.heard().is_empty(), "no redirect was followed: {second:?}");
}

fn engine2(t: &Titled, n: u32) -> HlsEngine {
    HlsEngine::new(t.dir.path().join(format!("hls{n}")), 2, 0)
}

#[test]
fn a_grant_opens_the_one_stub_it_was_made_for_from_loopback_only() {
    let loopback = Loopback::new();
    let sources = std::sync::Weak::new();
    let (_, grant) = loopback.grant(&sources, "/lib/a.kremote").unwrap();
    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let table = loopback.table();
    let secret = loopback.secret().to_string();

    let granted = authorise(table, &secret, peer, &secret, &grant.id());
    let other = authorise(table, &secret, peer, &secret, "some-other-grant");
    let remote = authorise(table, &secret, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 9)), &secret, &grant.id());
    let guessed = authorise(table, &secret, peer, "guess", &grant.id());

    assert_eq!(granted.as_deref(), Some("/lib/a.kremote"));
    assert_eq!(other, None);
    assert_eq!(remote, None, "a peer that is not loopback is refused");
    assert_eq!(guessed, None, "the per-process secret is required");
}

#[test]
fn a_grant_stops_working_when_its_session_lets_go_of_it() {
    let loopback = Loopback::new();
    let (_, grant) = loopback.grant(&std::sync::Weak::new(), "/lib/a.kremote").unwrap();
    let (secret, id) = (loopback.secret().to_string(), grant.id());
    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert!(authorise(loopback.table(), &secret, peer, &secret, &id).is_some());

    drop(grant);

    assert_eq!(authorise(loopback.table(), &secret, peer, &secret, &id), None);
    assert!(loopback.table().is_empty());
}

#[test]
fn a_grant_expires_even_while_it_is_held() {
    let loopback = Loopback::lasting(Duration::from_millis(50));
    let (_, grant) = loopback.grant(&std::sync::Weak::new(), "/lib/a.kremote").unwrap();
    let (secret, id) = (loopback.secret().to_string(), grant.id());

    std::thread::sleep(Duration::from_millis(120));

    let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert_eq!(authorise(loopback.table(), &secret, peer, &secret, &id), None);
}

#[test]
fn the_table_of_a_fresh_relay_is_empty() {
    assert!(Table::new().is_empty());
}

#[tokio::test]
async fn the_relay_answers_a_stranger_with_nothing_and_a_grant_holder_with_the_title() {
    let t = titled();
    let MediaInput::Remote(relayed) = t.input() else { panic!("relayed") };
    let url = kroma_primitives::input::media(&MediaInput::Remote(relayed.clone()))
        .last()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let stranger = url.replacen(url.split('/').nth(3).unwrap(), "not-the-secret", 1);

    let (held, refused) = tokio::task::spawn_blocking(move || {
        let held = kroma_http::Loopback::new().header("range", "bytes=0-9").get(&url).map(|r| r.status);
        let refused = kroma_http::Loopback::new().get(&stranger).map(|r| r.status);
        (held.unwrap(), refused.unwrap())
    })
    .await
    .unwrap();

    assert_eq!(held, 206);
    assert_eq!(refused, 404);
    drop(relayed);
}

#[test]
fn env_proxies_are_taken_off_the_children_that_read_the_relay() {
    let t = titled();
    let input = t.input();
    std::env::set_var("http_proxy", "http://127.0.0.1:1");
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:1");

    let probed = probe_outcome(&input, "mp4", true);

    std::env::remove_var("http_proxy");
    std::env::remove_var("HTTP_PROXY");
    assert!(matches!(probed, Probed::Done(_)), "{probed:?}");
}
