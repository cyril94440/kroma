use serde_json::json;

use super::support::{fail, run_left};
use crate::testing::{Rig, Scripted};

#[test]
fn a_turn_with_less_than_fifty_seconds_left_is_not_given_a_scan() {
    let rig = Rig::new();

    let (out, ctx) = run_left(&rig, &Scripted::new(Vec::new()), "benchmark_channels", json!({ "query": "ligue1+" }), "Mamie", Some(45.0));

    assert_eq!(out["status"], "refus");
    assert!(out["raison"].as_str().unwrap().contains("plus assez de temps"));
    assert_eq!(ctx.scans, 0);
    assert!(rig.control.benchmark_ids.lock().unwrap().is_empty());
}

#[test]
fn an_unknown_provider_name_is_refused_before_any_catalogue_call() {
    let rig = Rig::new();

    let (out, _) = run_left(&rig, &Scripted::new(Vec::new()), "benchmark_channels", json!({ "query": "tf1", "fournisseur": "lune" }), "Mamie", None);

    assert_eq!(out["ok"], false);
    assert!(rig.control.resolve_calls.lock().unwrap().is_empty());
}

#[test]
fn a_live_module_without_the_benchmark_point_gives_a_failure_and_hides_the_tool() {
    let rig = Rig::new();
    fail(&rig.control.benchmark, "HTTP 404: not found");

    let (out, ctx) = run_left(&rig, &Scripted::new(Vec::new()), "benchmark_channels", json!({ "query": "tf1" }), "Mamie", None);

    assert!(out["erreur"].as_str().unwrap().contains("indisponible"));
    assert!(ctx.pending.is_none());
    assert!(!rig.deps.supported("benchmark", rig.now()));
}

#[test]
fn a_started_scan_waits_for_its_follow_up_instead_of_answering() {
    let rig = Rig::new();
    super::support::reply(&rig.control.benchmark, crate::peers::BenchmarkStart { status: "started".into(), id: Some("7".into()), reason: None, total: 1, already_done: false });

    let (_, ctx) = run_left(&rig, &Scripted::new(Vec::new()), "benchmark_channels", json!({ "query": "tf1" }), "Mamie", Some(150.0));

    let job = ctx.pending.unwrap();
    assert_eq!((job.id.as_str(), job.provider.as_str(), job.budget_ms), ("7", "primary", 120_000));
    assert_eq!(ctx.scans, 1);
}
