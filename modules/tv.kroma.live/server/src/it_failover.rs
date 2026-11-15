use kroma_module_sdk::host::HostCtx;
use std::time::Duration;

use serde_json::json;

use crate::catalogue::Catalogue;
use crate::db;
use crate::it_live::{load, request, rig, rig_with, until, Rig};
use crate::it_stream::{playing, pump};
use crate::model::{Channel, ProviderId};
use crate::runtime::now_ms;

const PARTY: &str = "tv.kroma.party/room";

fn with_party(rig_host: kroma_module_sdk::host::testing::StubHost) -> kroma_module_sdk::host::testing::StubHost {
    rig_host.with_point(PARTY, None, "http://127.0.0.1:1", "t")
}

fn notified(rig: &Rig, kind: &str) -> usize {
    rig.host.notifications().iter().filter(|(_, spec)| serde_json::to_value(spec).unwrap()["type"] == kind).count()
}

fn add_variant(rig: &Rig) {
    let mut channels = db::load_channels(&rig.pool, ProviderId::Primary).unwrap();
    channels.push(Channel::new(ProviderId::Primary, 11, "VIP: TF1 ᴿᴬᵂ", Some("TF1.fr"), "General"));
    db::replace_channels(&rig.pool, ProviderId::Primary, &channels, now_ms()).unwrap();
    rig.live.providers.forget();
    assert!(Catalogue::new(channels).by_stream_id(11).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_proposal_goes_to_a_vote_when_the_party_module_is_there_and_its_answer_is_published() {
    let rig = rig_with(Default::default(), with_party);
    load(&rig);
    playing(&rig, "p:1").await;
    let backup = rig.live.providers.find_by_ref("s:1").unwrap();

    let outcome = rig.live.propose_channel("switchToBackup", &backup, "unstable").await;

    assert_eq!(outcome, crate::runtime::Proposed::Voting);
    let opened = &rig.live.sent_on("live.proposal")[0];
    assert_eq!((opened["resolved"].clone(), opened["proposal"]["kind"].clone(), opened["proposal"]["channel"].clone()), (json!(false), json!("switchToBackup"), json!("s:1")));
    let id = opened["proposal"]["id"].as_str().unwrap().to_string();
    assert!(rig.live.answer_proposal(&id, true).await);
    let closed = &rig.live.sent_on("live.proposal")[1];
    assert_eq!((closed["resolved"].clone(), closed["accepted"].clone(), closed["default"].clone()), (json!(true), json!(true), json!("vote")));
    assert!(rig.live.status().on_backup);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unanswered_proposal_expires_as_a_refusal() {
    let rig = rig_with(Default::default(), with_party);
    load(&rig);
    playing(&rig, "p:1").await;
    let backup = rig.live.providers.find_by_ref("s:1").unwrap();
    rig.live.propose_channel("switchToBackup", &backup, "unstable").await;
    for proposal in rig.live.lock().proposals.values_mut() {
        proposal.expires_at = 0;
    }

    rig.live.expire_proposals();

    let closed = &rig.live.sent_on("live.proposal")[1];
    assert_eq!((closed["resolved"].clone(), closed["accepted"].clone(), closed["default"].clone()), (json!(true), json!(false), json!("timeout")));
    assert!(rig.live.lock().proposals.is_empty() && !rig.live.status().on_backup);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_party_module_a_proposal_is_accepted_for_one_viewer_and_declined_for_more() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;
    let backup = rig.live.providers.find_by_ref("s:1").unwrap();
    rig.live.lock().presence.beat("u1", "Ann", now_ms());
    rig.live.lock().presence.beat("u2", "Bob", now_ms());

    let declined = rig.live.propose_channel("switchToBackup", &backup, "unstable").await;

    assert_eq!(declined, crate::runtime::Proposed::Declined);
    let first = &rig.live.sent_on("live.proposal")[0];
    assert_eq!((first["accepted"].clone(), first["default"].clone()), (json!(false), json!("noVote")));
    rig.live.lock().presence.leave("u2", now_ms());
    assert_eq!(rig.live.propose_channel("switchToBackup", &backup, "unstable").await, crate::runtime::Proposed::Applied);
    assert!(rig.live.status().on_backup);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_channel_is_published_when_restored_at_start_on_resume_and_again_once_it_is_ready() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:3").await;
    rig.live.shutdown();
    let reborn = crate::runtime::Live::new(std::sync::Arc::new(rig.host.clone()), rig.pool.clone(), rig.host.data_dir().join("live"), Default::default());
    reborn.providers.forget();

    reborn.boot().await;

    assert_eq!(reborn.sent_on("live.channel")[0]["channel"]["id"], "p:3");
    reborn.pause().await;
    let before = reborn.sent_on("live.channel").len();
    reborn.resume().await;
    assert_eq!(reborn.sent_on("live.channel").len(), before + 1);
    assert!(until(Duration::from_secs(30), || {
        let live = reborn.clone();
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(live.tick()));
        reborn.sent_on("live.channel").last().is_some_and(|e| e["playlist"].is_string())
    })
    .await);
    reborn.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dead_primary_in_front_of_a_viewer_fails_over_to_a_verified_backup_channel_and_says_so() {
    let rig = rig_with(Default::default(), |h| h.with_config("deadAfterSeconds", json!(15)));
    load(&rig);
    playing(&rig, "p:1").await;
    rig.live.lock().presence.beat("u1", "Ann", now_ms());
    rig.primary.broken.store(true, std::sync::atomic::Ordering::SeqCst);

    let moved = pump(&rig, Duration::from_secs(150), |r| {
        r.live.lock().presence.beat("u1", "Ann", now_ms());
        r.live.status().on_backup
    })
    .await;

    assert!(moved, "{:?}", rig.live.status());
    let failover = rig.live.sent_on("live.failover");
    let event = failover.iter().find(|e| e["kind"] == "failover").unwrap();
    assert_eq!((event["from"].clone(), event["to"].clone(), event["channel"].clone()), (json!("primary"), json!("backup"), json!("FR: TF1 FHD")));
    assert_eq!(notified(&rig, "tv.kroma.live/failover"), 1);
    assert!(db::provider_events(&rig.pool).unwrap().iter().any(|e| e.kind == "failover"));
    assert!(rig.live.sent_on("live.channel").iter().any(|e| e["onBackup"] == true));
    assert!(rig.live.primary_down_for_test());
    assert!(!rig.live.trace_lines(50).is_empty(), "the observations are traced");
    let replay = crate::failover::trace::replay(rig.live.trace_lines(100).iter().map(String::as_str).collect::<Vec<_>>());
    assert!(replay.iter().any(|r| r.recorded == Some(crate::failover::Action::Failover)));
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cut_channel_with_a_live_canary_moves_to_another_variant_then_stops_and_speaks_once() {
    let rig = rig_with(Default::default(), |h| h.with_config("canaries", json!("FR: M6 HD")));
    load(&rig);
    add_variant(&rig);
    playing(&rig, "p:1").await;
    let current = rig.live.current().unwrap();
    assert!(rig.live.canary_alive(Some(&current)).await);

    rig.live.channel_down_locally(&current, 1_000_000.0, None).await;

    let first = rig.live.sent_on("live.failover");
    assert_eq!((first[0]["kind"].clone(), first[0]["variant"].clone(), first[0]["budgetSpent"].clone()), (json!("channelDown"), json!("VIP: TF1 ᴿᴬᵂ"), json!(false)));
    assert_eq!(rig.live.status().channel.unwrap().id, "p:11");
    assert_eq!(notified(&rig, "tv.kroma.live/channel-down"), 1);
    let second_current = rig.live.current().unwrap();
    rig.live.channel_down_locally(&second_current, 1_000_060.0, None).await;
    let second = rig.live.sent_on("live.failover");
    assert_eq!((second[1]["variant"].clone(), second[1]["familyTried"].clone()), (json!(null), json!(true)));
    rig.live.channel_down_locally(&second_current, 1_000_120.0, None).await;
    assert_eq!(rig.live.sent_on("live.failover").len(), 2, "one message per ten minutes");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_canary_is_never_the_channel_in_trouble_and_a_dead_provider_has_no_live_canary() {
    let rig = rig_with(Default::default(), |h| h.with_config("canaries", json!("FR: TF1 FHD")));
    load(&rig);
    let tf1 = rig.live.providers.find_by_ref("p:1").unwrap();

    assert!(!rig.live.canary_alive(Some(&tf1)).await, "the only canary is the channel itself");
    assert!(rig.live.canary_alive(None).await);
    rig.primary.broken.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(!rig.live.canary_alive(None).await);
    assert!(db::provider_events(&rig.pool).unwrap().iter().any(|e| e.kind == "canaryDead"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_favourites_coverage_flags_gaps_once_and_a_newly_lost_channel_alerts_the_administrators() {
    let rig = rig();
    load(&rig);
    let first = rig.live.coverage();
    let gaps = first.gaps.clone();
    assert_eq!(gaps.get("France 2").map(|g| g.as_str()), Some("notFound"));
    assert!(!gaps.contains_key("TF1") && !gaps.contains_key("M6"));
    let events = |rig: &Rig| db::provider_events(&rig.pool).unwrap().iter().filter(|e| e.kind == "coverageGap").count();
    let after_first = events(&rig);
    assert!(after_first >= 1);

    rig.live.recheck_coverage();
    assert_eq!(events(&rig), after_first, "an unchanged gap is not reported again");
    let mut backup = db::load_channels(&rig.pool, ProviderId::Backup).unwrap();
    backup.retain(|c| !c.name.contains("TF1"));
    db::replace_channels(&rig.pool, ProviderId::Backup, &backup, now_ms()).unwrap();
    rig.live.providers.forget();

    rig.live.recheck_coverage();

    assert_eq!(rig.live.coverage().gaps.get("TF1").map(|g| g.as_str()), Some("absentFromBackup"));
    assert_eq!(events(&rig), after_first + 1);
    let (audience, _) = rig.host.notifications().into_iter().find(|(_, spec)| serde_json::to_value(spec).unwrap()["type"] == "tv.kroma.live/coverage-gap").unwrap();
    assert_eq!(audience, kroma_module_sdk::host::Audience::permission(kroma_module_sdk::domain::Permission::SettingsManage));
    let _ = request("p:1");
}
