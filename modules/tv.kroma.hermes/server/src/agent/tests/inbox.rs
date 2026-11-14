use serde_json::json;

use crate::peers::RoomMessage;
use crate::store::State;
use crate::testing::{Rig, Scripted, Step};

fn model(rig: &Rig, steps: usize) -> Scripted {
    let model = Scripted::new((0..steps).map(|_| Step::Calls(vec![("reply", json!({ "message": "Voilà." }))])).collect());
    rig.use_model(std::sync::Arc::new(model.clone()));
    model
}

#[test]
fn the_same_event_delivered_twice_is_answered_once() {
    let rig = Rig::new();
    let model = model(&rig, 3);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    assert_eq!(rig.room.texts(), vec!["Voilà.".to_string()]);
    assert_eq!(model.0.opened.lock().unwrap().len(), 1);
}

#[test]
fn an_answered_message_found_again_by_the_reconciliation_is_not_answered_twice() {
    let rig = Rig::new();
    let model = model(&rig, 3);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    rig.history_user("m-1", "Mamie", "mets tf1");
    assert_eq!(rig.agent.reconcile(), 0);
    rig.drain();
    assert_eq!(model.0.opened.lock().unwrap().len(), 1);
}

#[test]
fn a_message_the_event_missed_is_recovered_from_the_history_and_answered_once() {
    let rig = Rig::new();
    let model = model(&rig, 3);
    rig.history_user("m-7", "Papi", "mets arte");
    assert_eq!(rig.agent.reconcile(), 1);
    assert_eq!(rig.agent.reconcile(), 0);
    rig.drain();
    assert_eq!(rig.room.texts(), vec!["Voilà.".to_string()]);
    assert_eq!(model.0.opened.lock().unwrap().len(), 1);
    assert!(model.0.opened.lock().unwrap()[0].1.contains("Papi: mets arte"));
}

#[test]
fn the_timer_reconciles_without_being_asked() {
    let rig = Rig::new();
    model(&rig, 3);
    rig.history_user("m-9", "Papi", "mets arte");
    rig.drain();
    assert_eq!(rig.room.texts(), vec!["Voilà.".to_string()]);
}

#[test]
fn reconciliation_ignores_what_is_old_what_is_the_agents_and_what_is_not_for_it() {
    let rig = Rig::new();
    let model = model(&rig, 3);
    rig.room.push_history(RoomMessage { id: "old".into(), user_id: Some("u-mamie".into()), name: "Mamie".into(), kind: "user".into(), text: "mets tf1".into(), at: rig.now() - 600_000 });
    rig.room.push_history(RoomMessage { id: "bot".into(), user_id: None, name: "Hermès".into(), kind: "agent".into(), text: "Je mets TF1 ?".into(), at: rig.now() - 200_000 });
    rig.room.push_history(RoomMessage { id: "chat".into(), user_id: Some("u-papi".into()), name: "Papi".into(), kind: "user".into(), text: "on mange quoi".into(), at: rig.now() });
    assert_eq!(rig.agent.reconcile(), 0);
    rig.drain();
    assert!(model.0.opened.lock().unwrap().is_empty());
}

#[test]
fn a_message_in_the_queue_survives_a_restart_but_an_active_one_is_never_redone() {
    let rig = Rig::new();
    let model = model(&rig, 3);
    let queued = rig.incoming("q", "Mamie", "mets tf1");
    let active = rig.incoming("a", "Papi", "mets m6");
    rig.deps.store.claim(&queued, State::Queued, None, rig.now()).unwrap();
    rig.deps.store.claim(&active, State::Active, None, rig.now()).unwrap();
    rig.deps.store.interrupt_active(rig.now()).unwrap();
    rig.drain();
    assert_eq!(model.0.opened.lock().unwrap().len(), 1);
    assert!(model.0.opened.lock().unwrap()[0].1.contains("Mamie: mets tf1"));
}

#[test]
fn messages_are_answered_in_arrival_order_one_at_a_time() {
    let rig = Rig::new();
    let model = model(&rig, 4);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.advance(1000);
    rig.hear("m-2", "Papi", "mets m6");
    rig.drain();
    let opened = model.0.opened.lock().unwrap().clone();
    assert!(opened[0].1.contains("Mamie: mets tf1") && opened[1].1.contains("Papi: mets m6"));
    assert_eq!(rig.turns().len(), 2);
}

#[test]
fn the_maintenance_job_reconciles_and_prunes_old_records() {
    let rig = Rig::new();
    model(&rig, 3);
    rig.deps.store.record("chat", "old", &json!({}), rig.now() - 40 * 86_400_000);
    rig.deps.store.claim(&rig.incoming("done", "Mamie", "x"), State::Done, None, rig.now() - 9 * 86_400_000).unwrap();
    rig.history_user("m-3", "Papi", "mets arte");
    rig.agent.maintain().unwrap();
    let events: Vec<String> = rig.deps.store.audit_rows(None, None, 50).unwrap().into_iter().map(|r| r.event).collect();
    assert!(!events.contains(&"old".to_string()));
    assert!(events.contains(&"maintenance".to_string()));
    assert_eq!(rig.deps.store.inbox_recent(10).unwrap().iter().filter(|r| r.message.id == "done").count(), 0);
    assert_eq!(rig.deps.store.next_queued().unwrap().unwrap().message.id, "m-3");
}
