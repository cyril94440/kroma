use serde_json::{json, Value};

use crate::it_live::{load, rig, rig_with};
use crate::it_stream::playing;
use crate::runtime::now_ms;

fn with_party(host: kroma_module_sdk::host::testing::StubHost) -> kroma_module_sdk::host::testing::StubHost {
    host.with_point("tv.kroma.party/room", None, "http://127.0.0.1:1", "t")
}

fn pinned(mut payload: Value) -> Value {
    payload["proposal"]["id"] = json!("prop-1");
    payload["proposal"]["createdAt"] = json!(1_000);
    payload["proposal"]["expiresAt"] = json!(31_000);
    payload
}

async fn opened_with_party() -> (crate::it_live::Rig, Value) {
    let rig = rig_with(Default::default(), with_party);
    load(&rig);
    playing(&rig, "p:1").await;
    let backup = rig.live.providers.find_by_ref("s:1").unwrap();
    rig.live.propose_channel("switchToBackup", &backup, "unstable").await;
    let opened = rig.live.sent_on("live.proposal")[0].clone();
    (rig, opened)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_opened_proposal_is_published_in_the_nested_wire_shape() {
    let (rig, opened) = opened_with_party().await;

    assert_eq!(
        pinned(opened),
        json!({
            "proposal": {
                "id": "prop-1",
                "kind": "switchToBackup",
                "text": "unstable",
                "channel": "s:1",
                "createdAt": 1000,
                "expiresAt": 31000
            },
            "resolved": false,
            "viewers": 0
        })
    );
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_proposal_answered_by_vote_is_published_resolved_with_the_answer() {
    let (rig, opened) = opened_with_party().await;
    let id = opened["proposal"]["id"].as_str().unwrap().to_string();

    rig.live.answer_proposal(&id, true).await;

    assert_eq!(
        pinned(rig.live.sent_on("live.proposal")[1].clone()),
        json!({
            "proposal": {
                "id": "prop-1",
                "kind": "switchToBackup",
                "text": "unstable",
                "channel": "s:1",
                "createdAt": 1000,
                "expiresAt": 31000
            },
            "resolved": true,
            "accepted": true,
            "default": "vote"
        })
    );
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn an_expired_proposal_is_published_resolved_as_a_timeout() {
    let (rig, _) = opened_with_party().await;
    for proposal in rig.live.lock().proposals.values_mut() {
        proposal.expires_at = now_ms() - 1;
    }

    rig.live.expire_proposals();

    assert_eq!(
        pinned(rig.live.sent_on("live.proposal")[1].clone()),
        json!({
            "proposal": {
                "id": "prop-1",
                "kind": "switchToBackup",
                "text": "unstable",
                "channel": "s:1",
                "createdAt": 1000,
                "expiresAt": 31000
            },
            "resolved": true,
            "accepted": false,
            "default": "timeout"
        })
    );
    rig.live.shutdown();
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_party_module_the_proposal_is_published_already_resolved() {
    let rig = rig();
    load(&rig);
    playing(&rig, "p:1").await;
    let backup = rig.live.providers.find_by_ref("s:1").unwrap();

    rig.live.propose_channel("switchToBackup", &backup, "unstable").await;

    assert_eq!(
        pinned(rig.live.sent_on("live.proposal")[0].clone()),
        json!({
            "proposal": {
                "id": "prop-1",
                "kind": "switchToBackup",
                "text": "unstable",
                "channel": "s:1",
                "createdAt": 1000,
                "expiresAt": 31000
            },
            "resolved": true,
            "accepted": true,
            "default": "noVote"
        })
    );
    rig.live.shutdown();
}
