use serde_json::json;

use super::super::error::VoiceError;
use super::super::limits::{MAX_CANDIDATE_BYTES, MAX_SDP_BYTES};
use super::super::signal::{Candidate, Description, Signal, SignalKind};

fn base(kind: SignalKind) -> Signal {
    Signal {
        from: "a-1".into(),
        to: "b-1".into(),
        session: "s1".into(),
        kind,
        description: None,
        candidate: None,
    }
}

fn offer(sdp: &str) -> Signal {
    Signal {
        description: Some(Description {
            kind: "offer".into(),
            sdp: sdp.into(),
        }),
        ..base(SignalKind::Description)
    }
}

fn candidate(line: &str) -> Signal {
    Signal {
        candidate: Some(Candidate {
            candidate: line.into(),
            sdp_mid: Some("0".into()),
            sdp_m_line_index: Some(0),
            username_fragment: None,
        }),
        ..base(SignalKind::Candidate)
    }
}

#[test]
fn accepts_an_offer_an_answer_a_candidate_an_end_marker_and_a_hello() {
    let answer = Signal {
        description: Some(Description {
            kind: "answer".into(),
            sdp: "v=0".into(),
        }),
        ..base(SignalKind::Description)
    };

    assert_eq!(offer("v=0").validate(), Ok(()));
    assert_eq!(answer.validate(), Ok(()));
    assert_eq!(
        candidate("candidate:1 1 udp 1 10.0.0.1 9 typ host").validate(),
        Ok(())
    );
    assert_eq!(candidate("").validate(), Ok(()));
    assert_eq!(base(SignalKind::Hello).validate(), Ok(()));
}

#[test]
fn refuses_a_description_that_is_empty_of_the_wrong_type_or_missing() {
    let rollback = Signal {
        description: Some(Description {
            kind: "rollback".into(),
            sdp: "v=0".into(),
        }),
        ..base(SignalKind::Description)
    };

    assert_eq!(offer("").validate(), Err(VoiceError::BadSignal));
    assert_eq!(rollback.validate(), Err(VoiceError::BadSignal));
    assert_eq!(
        base(SignalKind::Description).validate(),
        Err(VoiceError::BadSignal)
    );
    assert_eq!(
        base(SignalKind::Candidate).validate(),
        Err(VoiceError::BadSignal)
    );
}

#[test]
fn refuses_a_payload_that_does_not_belong_to_its_kind() {
    let hello = Signal {
        candidate: candidate("x").candidate,
        ..base(SignalKind::Hello)
    };
    let mixed = Signal {
        candidate: candidate("x").candidate,
        ..offer("v=0")
    };

    assert_eq!(hello.validate(), Err(VoiceError::BadSignal));
    assert_eq!(mixed.validate(), Err(VoiceError::BadSignal));
}

#[test]
fn refuses_ids_and_sessions_that_are_not_plain_tokens() {
    let odd = |field: &str| {
        let mut s = offer("v=0");
        match field {
            "from" => s.from = "a b".into(),
            "to" => s.to = String::new(),
            _ => s.session = "x".repeat(65),
        }
        s
    };

    for field in ["from", "to", "session"] {
        assert_eq!(odd(field).validate(), Err(VoiceError::BadSignal), "{field}");
    }
}

#[test]
fn caps_the_size_of_a_description_and_of_a_candidate() {
    let big_sdp = offer(&"a".repeat(MAX_SDP_BYTES + 1));
    let big_candidate = candidate(&"c".repeat(MAX_CANDIDATE_BYTES + 1));
    let edge = offer(&"a".repeat(MAX_SDP_BYTES));

    assert_eq!(big_sdp.validate(), Err(VoiceError::TooLarge));
    assert_eq!(big_candidate.validate(), Err(VoiceError::TooLarge));
    assert_eq!(edge.validate(), Ok(()));
}

#[test]
fn pins_the_json_of_a_relayed_description_and_candidate() {
    let description = offer("v=0").payload("mamie", 3);
    let line = candidate("candidate:1").payload("mamie", 4);

    assert_eq!(
        description,
        json!({
            "to": "b-1", "from": "a-1", "fromUserId": "mamie", "session": "s1", "n": 3,
            "kind": "description", "description": { "type": "offer", "sdp": "v=0" }
        })
    );
    assert_eq!(
        line,
        json!({
            "to": "b-1", "from": "a-1", "fromUserId": "mamie", "session": "s1", "n": 4,
            "kind": "candidate",
            "candidate": { "candidate": "candidate:1", "sdpMid": "0", "sdpMLineIndex": 0, "usernameFragment": null }
        })
    );
}

#[test]
fn reads_the_camel_case_body_a_client_sends() {
    let body = json!({
        "from": "a-1", "to": "b-1", "session": "s1", "kind": "candidate",
        "candidate": { "candidate": "candidate:1", "sdpMid": "0", "sdpMLineIndex": 0 }
    });

    let signal: Signal = serde_json::from_value(body).unwrap();

    assert_eq!(signal.kind, SignalKind::Candidate);
    assert_eq!(signal.candidate.unwrap().sdp_m_line_index, Some(0));
}
