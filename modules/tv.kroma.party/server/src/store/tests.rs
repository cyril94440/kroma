use super::*;
use crate::model::{ChatMessage, Kind, Person};

fn msg(id: &str, at: i64) -> ChatMessage {
    ChatMessage {
        id: id.into(),
        user_id: Some("u1".into()),
        name: "Ana".into(),
        kind: Kind::User,
        text: format!("hello {id}"),
        at,
        ..ChatMessage::default()
    }
}

#[test]
fn messages_round_trip_with_their_reactions_and_come_back_oldest_first() {
    let (s, _keep) = test_store("party-chat");
    let mut a = msg("a", 10);
    a.reactions.insert("x".into(), vec![Person::new("u2", "Bo")]);
    s.insert_message(&a).unwrap();
    s.insert_message(&msg("b", 20)).unwrap();
    s.insert_message(&msg("c", 30)).unwrap();

    let got = s.recent_messages(2, None, None).unwrap();
    assert_eq!(got.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["b", "c"]);
    let older = s.recent_messages(5, Some(20), None).unwrap();
    assert_eq!(older.len(), 1);
    assert_eq!(older[0].reactions["x"][0].name, "Bo");
    assert_eq!(s.recent_messages(5, None, Some(25)).unwrap().len(), 1);
}

#[test]
fn the_reaction_window_only_covers_the_latest_messages() {
    let (s, _keep) = test_store("party-window");
    for i in 0..5 {
        s.insert_message(&msg(&format!("m{i}"), i)).unwrap();
    }
    assert!(s.within_reaction_window("m4", 2).unwrap());
    assert!(!s.within_reaction_window("m0", 2).unwrap());
}

#[test]
fn old_messages_are_pruned() {
    let (s, _keep) = test_store("party-prune");
    s.insert_message(&msg("old", 5)).unwrap();
    s.insert_message(&msg("new", 500)).unwrap();
    assert_eq!(s.prune_messages(100).unwrap(), 1);
    assert!(s.message("old").unwrap().is_none());
    assert!(s.message("new").unwrap().is_some());
}

#[test]
fn the_kv_keeps_the_last_value_written() {
    let (s, _keep) = test_store("party-kv");
    assert_eq!(s.kv_get("k").unwrap(), None);
    s.kv_set("k", "1").unwrap();
    s.kv_set("k", "2").unwrap();
    assert_eq!(s.kv_get("k").unwrap().as_deref(), Some("2"));
}

#[test]
fn client_logs_are_filtered_and_pruned_by_age() {
    let (s, _keep) = test_store("party-logs");
    let entry = |at: i64, user: &str, cat: &str| LogEntry {
        at,
        user_id: user.into(),
        name: user.into(),
        device: None,
        category: cat.into(),
        event: "e".into(),
        data: None,
    };
    s.insert_logs(&[entry(1, "a", "player"), entry(2, "b", "ws"), entry(3, "a", "ws")]).unwrap();
    assert_eq!(s.logs(0, Some("a"), None, 10).unwrap().len(), 2);
    assert_eq!(s.logs(0, None, Some("ws"), 10).unwrap().len(), 2);
    assert_eq!(s.prune_logs(logs::LOG_RETENTION_MS + 3).unwrap(), 2);
    assert_eq!(s.logs(0, None, None, 10).unwrap().len(), 1);
}
