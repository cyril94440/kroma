use serde_json::json;

use crate::it_live::{load, rig, rig_with};
use crate::it_ops::{healthy, viewers};
use crate::it_points::ask;
use crate::it_stream::playing;
use crate::runtime::now_ms;

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_refuses_by_reason_asks_for_a_vote_with_others_and_switches_alone() {
    let rig = rig();
    load(&rig);
    let call = |provider: &str| json!({ "provider": provider, "requestedBy": "u1", "reason": "it stutters" });
    assert_eq!(ask(&rig, "switchProvider", call("backup")).await, json!({ "Ok": { "switched": false, "provider": "backup", "error": "noChannel" } }));
    assert_eq!(ask(&rig, "switchProvider", call("moon")).await, json!({ "Ok": { "switched": false, "provider": "moon", "error": "unknownProvider" } }));
    healthy(&rig).await;
    assert_eq!(ask(&rig, "switchProvider", call("primary")).await["Ok"]["error"], "alreadyOn");
    viewers(&rig, &["u1", "u2"]);

    let vote = ask(&rig, "switchProvider", call("backup")).await;
    rig.live.lock().presence.leave("u2", now_ms());
    let alone = ask(&rig, "switchProvider", call("backup")).await;

    assert_eq!(vote, json!({ "Ok": { "switched": false, "provider": "backup", "error": "voteRequired" } }));
    assert_eq!(rig.live.sent_on("live.proposal")[0]["proposal"]["kind"], "switchProvider");
    assert_eq!(alone, json!({ "Ok": { "switched": true, "provider": "backup", "error": null } }));
    assert!(rig.live.status().on_backup);
    assert_eq!(ask(&rig, "switchProvider", call("primary")).await["Ok"]["switched"], true, "back, alone");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_goes_at_once_when_the_stream_crashed_and_delivers_nothing() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;
    viewers(&rig, &["u1", "u2"]);
    {
        let mut inner = rig.live.lock();
        inner.restarts = 1;
        inner.kbps = 0.0;
    }

    let reply = ask(&rig, "switchProvider", json!({ "provider": "backup", "requestedBy": "u1", "reason": "it errors" })).await;

    assert_eq!(reply, json!({ "Ok": { "switched": true, "provider": "backup", "error": null } }));
    assert!(rig.live.sent_on("live.proposal").is_empty());
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_still_asks_a_vote_when_a_crashed_stream_flows_again() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;
    viewers(&rig, &["u1", "u2"]);
    rig.live.lock().restarts = 1;

    let reply = ask(&rig, "switchProvider", json!({ "provider": "backup", "requestedBy": "u1", "reason": "it errored" })).await;

    assert_eq!(reply["Ok"]["error"], "voteRequired");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_health_view_reports_trouble_as_soon_as_a_crashed_stream_is_silent() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;
    assert!(!rig.live.status().health.trouble);
    {
        let mut inner = rig.live.lock();
        inner.restarts = 1;
        inner.kbps = 0.0;
    }

    assert!(rig.live.status().health.trouble);
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_with_a_channel_id_moves_to_that_very_channel_under_the_same_vote_rule() {
    let rig = rig();
    load(&rig);
    healthy(&rig).await;
    let call = |channel: &str| json!({ "provider": "backup", "channelId": channel, "requestedBy": "u1", "reason": "the match" });
    assert_eq!(ask(&rig, "switchProvider", call("p:3")).await["Ok"]["error"], "providerMismatch");
    assert_eq!(ask(&rig, "switchProvider", call("s:999")).await["Ok"]["error"], "unknownChannel");
    viewers(&rig, &["u1", "u2"]);

    let vote = ask(&rig, "switchProvider", call("s:3")).await;
    rig.live.lock().presence.leave("u2", now_ms());
    let alone = ask(&rig, "switchProvider", call("s:3")).await;

    assert_eq!(vote, json!({ "Ok": { "switched": false, "provider": "backup", "error": "voteRequired" } }));
    assert_eq!(rig.live.sent_on("live.proposal")[0]["proposal"]["channel"], "s:3");
    assert_eq!(alone, json!({ "Ok": { "switched": true, "provider": "backup", "error": null } }));
    assert_eq!(rig.live.status().channel.unwrap().id, "s:3");
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn switch_provider_says_when_there_is_no_backup_or_the_main_provider_is_down() {
    let rig = rig_with(Default::default(), |h| h.with_config("backupUrl", json!("")));
    load(&rig);
    playing(&rig, "p:1").await;
    assert_eq!(ask(&rig, "switchProvider", json!({ "provider": "backup", "requestedBy": "u1", "reason": "" })).await["Ok"]["error"], "noBackup");
    rig.live.shutdown();
    let down = crate::it_live::rig();
    load(&down);
    playing(&down, "s:1").await;
    down.live.lock().policy.mark_primary_down(1.0);

    let reply = ask(&down, "switchProvider", json!({ "provider": "primary", "requestedBy": "u1", "reason": "" })).await;

    assert_eq!(reply["Ok"]["error"], "primaryDown");
    down.live.shutdown();
}
