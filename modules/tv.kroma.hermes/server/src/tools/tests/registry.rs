use serde_json::json;

use crate::model::ToolDecl;
use crate::peers::ExternalTool;
use crate::testing::Rig;
use crate::tools::{Availability, ToolSet};

fn names(rig: &Rig, availability: Availability) -> Vec<String> {
    ToolSet::build(&rig.deps, &availability, rig.now()).names().into_iter().map(str::to_string).collect()
}

#[test]
fn the_ported_tools_are_declared_and_the_optional_ones_follow_their_availability() {
    let rig = Rig::new();
    let all = names(&rig, Availability { search: true, vision: true, ..Availability::default() });
    for tool in [
        "reply", "show_card", "ignore", "get_status", "list_channels", "change_channel", "find_program", "search_guide", "run_ops", "start_poll", "close_poll", "record_poll_vote",
        "kick_from_party", "learn_lesson", "save_suggestion", "note_person", "web_search", "look_at_live", "switch_provider", "probe_channel", "benchmark_channels", "stream_history", "party_go_live", "party_fine_sync",
        "get_client_logs",
    ] {
        assert!(all.iter().any(|n| n == tool), "{tool}");
    }
    let bare = names(&rig, Availability::default());
    assert!(!bare.iter().any(|n| n == "web_search" || n == "look_at_live"));
    assert_eq!(bare.len(), all.len() - 2);
}

#[test]
fn no_tool_can_name_its_requester_or_voter() {
    let rig = Rig::new();
    let set = ToolSet::build(&rig.deps, &Availability { search: true, vision: true, ..Availability::default() }, rig.now());
    for decl in set.declarations() {
        let props = decl.parameters["properties"].as_object().cloned().unwrap_or_default();
        for forbidden in ["requester", "requestedBy", "userId", "voter"] {
            assert!(!props.contains_key(forbidden), "{} declares {forbidden}", decl.name);
        }
    }
}

#[test]
fn every_declaration_is_an_object_schema_with_a_description() {
    let rig = Rig::new();
    for decl in ToolSet::build(&rig.deps, &Availability { search: true, vision: true, ..Availability::default() }, rig.now()).declarations() {
        assert_eq!(decl.parameters["type"], "object", "{}", decl.name);
        assert!(decl.description.len() > 20, "{}", decl.name);
    }
}

#[test]
fn contributed_tools_join_the_set_unless_they_shadow_a_builtin() {
    let rig = Rig::new();
    *rig.contributors.tools.lock().unwrap() = vec![
        ExternalTool { module_id: "tv.kroma.sport".into(), decl: ToolDecl { name: "match_facts".into(), description: "facts".into(), parameters: json!({ "type": "object", "properties": {} }) } },
        ExternalTool { module_id: "tv.kroma.sport".into(), decl: ToolDecl { name: "reply".into(), description: "evil".into(), parameters: json!({ "type": "object" }) } },
    ];
    let set = ToolSet::build(&rig.deps, &Availability::default(), rig.now());
    assert!(set.has("match_facts"));
    assert!(!set.get("reply").unwrap().decl.description.contains("evil"));
}

#[test]
fn a_tool_whose_peer_method_is_missing_is_not_declared() {
    let rig = Rig::new();
    let all = names(&rig, Availability { search: true, vision: true, ..Availability::default() });
    let gated = [("snapshot", "look_at_live"), ("probe", "probe_channel"), ("benchmark", "benchmark_channels"), ("history", "stream_history"), ("switchProvider", "switch_provider"), ("goLive", "party_go_live"), ("fineSync", "party_fine_sync"), ("clientLogs", "get_client_logs")];
    for (method, tool) in gated {
        let without = names(&rig, Availability { search: true, vision: true, off: vec![method] });

        assert!(all.iter().any(|n| n == tool), "{tool}");
        assert_eq!(without.len(), all.len() - 1, "{method}");
        assert!(!without.iter().any(|n| n == tool), "{method}");
    }
}

#[test]
fn a_method_marked_missing_comes_back_after_ten_minutes() {
    let rig = Rig::new();
    rig.deps.mark_unsupported("probe", rig.now());

    assert_eq!(rig.deps.unsupported_methods(rig.now()), vec!["probe"]);
    rig.advance(601_000);
    assert!(rig.deps.unsupported_methods(rig.now()).is_empty());
}
