use serde_json::json;

use super::*;

fn message(id: &str, at: i64) -> Incoming {
    Incoming { id: id.into(), at, user_id: "u1".into(), name: "Ana".into(), text: format!("texte {id}") }
}

#[test]
fn a_message_is_claimed_once_whatever_its_state() {
    let (s, _keep) = test_store("hermes-claim");
    assert!(s.claim(&message("a", 10), State::Queued, None, 11).unwrap());
    assert!(!s.claim(&message("a", 10), State::Skipped, Some("x"), 12).unwrap());
    assert!(s.claim(&message("b", 20), State::Skipped, Some("not_for_agent"), 21).unwrap());
    assert_eq!(s.next_queued().unwrap().unwrap().message.id, "a");
    s.set_state("a", State::Done, Some("terminal"), 30).unwrap();
    assert!(s.next_queued().unwrap().is_none());
}

#[test]
fn the_queue_is_oldest_first() {
    let (s, _keep) = test_store("hermes-queue");
    s.claim(&message("late", 50), State::Queued, None, 1).unwrap();
    s.claim(&message("early", 5), State::Queued, None, 1).unwrap();
    assert_eq!(s.next_queued().unwrap().unwrap().message.id, "early");
}

#[test]
fn an_active_turn_is_marked_interrupted_after_a_restart_and_never_requeued() {
    let (s, _keep) = test_store("hermes-restart");
    s.claim(&message("a", 1), State::Active, None, 1).unwrap();
    s.claim(&message("b", 2), State::Queued, None, 1).unwrap();
    assert_eq!(s.interrupt_active(5).unwrap(), 1);
    assert_eq!(s.next_queued().unwrap().unwrap().message.id, "b");
    let rows = s.inbox_recent(10).unwrap();
    assert_eq!(rows.iter().find(|r| r.message.id == "a").unwrap().state, "interrupted");
}

#[test]
fn engaged_turns_per_user_are_counted_in_a_window() {
    let (s, _keep) = test_store("hermes-rate");
    for (i, state) in [State::Done, State::Done, State::Skipped, State::Queued].into_iter().enumerate() {
        s.claim(&message(&format!("m{i}"), i as i64), state, None, 100).unwrap();
    }
    assert_eq!(s.engaged_since("u1", 50).unwrap(), 2);
    assert_eq!(s.engaged_since("u1", 150).unwrap(), 0);
}

#[test]
fn lessons_and_suggestions_are_capped_fifos() {
    let (s, _keep) = test_store("hermes-fifo");
    for i in 0..5 {
        s.add_lesson(&format!("l{i}"), i, 3).unwrap();
        s.add_suggestion("bug", &format!("t{i}"), "detail", i, 2).unwrap();
    }
    assert_eq!(s.lessons().unwrap().iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), vec!["l2", "l3", "l4"]);
    assert_eq!(s.suggestions().unwrap().iter().map(|l| l.title.as_str()).collect::<Vec<_>>(), vec!["t4", "t3"]);
    let id = s.lessons().unwrap()[0].id;
    assert!(s.delete_lesson(id).unwrap());
    assert!(!s.delete_lesson(id).unwrap());
}

#[test]
fn notes_are_capped_per_person() {
    let (s, _keep) = test_store("hermes-notes");
    for i in 0..5 {
        s.add_note("u1", "Ana", &format!("note {i}"), i, 3).unwrap();
    }
    s.add_note("u2", "Bo", "autre", 9, 3).unwrap();
    assert_eq!(s.notes("u1").unwrap(), vec!["note 2".to_string(), "note 3".to_string(), "note 4".to_string()]);
    assert_eq!(s.all_notes().unwrap().len(), 4);
    assert_eq!(s.forget_person("u1").unwrap(), 3);
}

#[test]
fn the_audit_log_filters_pages_and_prunes() {
    let (s, _keep) = test_store("hermes-audit");
    assert!(s.record("chat", "gate", &json!({ "a": 1 }), 1_000));
    assert!(s.record("llm", "call", &json!({ "ms": 5 }), 2_000));
    assert!(s.record("chat", "gate", &json!({ "a": 2 }), 3_000));
    assert!(!s.record("Bad Genre!", "x", &json!({}), 1));
    let all = s.audit_rows(None, None, 10).unwrap();
    assert_eq!(all.iter().map(|r| r.data["a"].as_i64().unwrap_or(0)).collect::<Vec<_>>(), vec![2, 0, 1]);
    assert_eq!(s.audit_rows(Some("chat"), None, 10).unwrap().len(), 2);
    assert_eq!(s.audit_rows(None, Some(all[0].id), 10).unwrap().len(), 2);
    assert_eq!(s.audit_genres().unwrap(), vec!["chat".to_string(), "llm".to_string()]);
    assert_eq!(s.audit_count_since("chat", "gate", 2_000).unwrap(), 1);
    assert_eq!(s.prune_audit(2_500).unwrap(), 2);
}

#[test]
fn an_oversized_line_is_truncated_and_a_full_day_stops_writing() {
    let (s, _keep) = test_store("hermes-caps");
    assert!(s.record("chat", "big", &json!({ "text": "x".repeat(100_000) }), 5));
    let row = &s.audit_rows(Some("chat"), None, 1).unwrap()[0];
    assert!(row.data["truncated"].as_u64().unwrap() > 100_000);
    let conn = s.pool.get().unwrap();
    conn.execute("INSERT INTO audit (at, day, genre, event, bytes, data) VALUES (5, 0, 'full', 'e', 41943040, '{}')", []).unwrap();
    drop(conn);
    assert!(!s.record("full", "e", &json!({ "a": 1 }), 5));
    assert!(s.record("full", "e", &json!({ "a": 1 }), 86_400_000 * 2));
}

#[test]
fn the_key_value_store_round_trips() {
    let (s, _keep) = test_store("hermes-kv");
    assert_eq!(s.kv_get("k").unwrap(), None);
    s.kv_set("k", "v1").unwrap();
    s.kv_set("k", "v2").unwrap();
    assert_eq!(s.kv_get("k").unwrap().as_deref(), Some("v2"));
    s.kv_delete("k").unwrap();
    assert_eq!(s.kv_get("k").unwrap(), None);
}

#[test]
fn every_genre_together_stops_writing_for_the_day_at_the_total_cap() {
    let (s, _keep) = test_store("hermes-total-cap");
    let conn = s.pool.get().unwrap();
    for genre in ["chat", "decisions", "sources"] {
        conn.execute("INSERT INTO audit (at, day, genre, event, bytes, data) VALUES (5, 0, ?1, 'e', 41943000, '{}')", [genre]).unwrap();
    }
    drop(conn);

    assert!(!s.record("llm", "e", &json!({ "text": "x".repeat(200) }), 5));
    assert!(s.record("llm", "e", &json!({ "a": 1 }), 5));
    assert!(s.record("llm", "e", &json!({ "a": 1 }), 86_400_000 * 2));
}
