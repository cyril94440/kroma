use serde_json::json;

use super::declared;

fn names() -> Vec<String> {
    declared()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_declarations_keep_the_source_names_and_schemas() {
    let tools = declared();
    let find = |name: &str| tools.iter().find(|t| t["name"] == name).unwrap().clone();

    assert_eq!(
        names(),
        [
            "match_facts",
            "compos_match",
            "live_standings",
            "start_prono",
            "sport_channel_hint",
            "calibrate_clock"
        ]
    );
    assert_eq!(
        find("match_facts")["parameters"],
        json!({"type": "object", "properties": {"equipes": {"type": "string", "description": "ex: 'Lens - Lille' ou 'PSG'"}, "competition": {"type": "string", "description": "facultatif, ex: 'Ligue 1' (sinon toutes les compétitions)"}}})
    );
    assert_eq!(
        find("live_standings")["parameters"]["required"],
        json!(["competition"])
    );
    assert_eq!(
        find("start_prono")["parameters"]["required"],
        json!(["home", "away"])
    );
    assert!(find("compos_match")["description"]
        .as_str()
        .unwrap()
        .starts_with("COMPOSITIONS RÉELLES d'un match"));
    assert!(tools.iter().all(|t| t["parameters"]["type"] == "object"
        && t["description"].as_str().is_some_and(|d| !d.is_empty())));
}

#[cfg(not(target_family = "wasm"))]
mod with_rig {
    use axum::Extension;
    use kroma_module_sdk::host::call_point;
    use kroma_module_sdk::testing::{blocking, serve};
    use serde_json::{json, Value};

    use crate::espn::scoreboard;
    use crate::runtime::tests::support::{villarreal_summary, Rig};
    use crate::tools::{routes, TOOLS};

    #[tokio::test]
    async fn the_hermes_tools_point_lists_the_tools_and_answers_a_call_with_the_exact_json_it_sends(
    ) {
        let rig = Rig::new("2026-11-03T20:11:00Z");
        let now = rig.now();
        let board = crate::runtime::tests::support::scoreboard_at(now);
        for day in [
            crate::iso::day_key(now - 86_400.0),
            crate::iso::day_key(now),
            crate::iso::day_key(now + 86_400.0),
        ] {
            rig.espn.serve(&scoreboard("all", &day), &board);
        }
        rig.serve_summary(&villarreal_summary());
        let router = routes::<kroma_module_sdk::host::testing::StubHost>()
            .layer(Extension(rig.sport.clone()))
            .with_state(rig.host.clone());
        let resolver = serve(router, ()).await;

        let (listed, answered, unknown) = blocking(move || {
            let listed: Value = call_point(&resolver, TOOLS, "list", &json!({})).unwrap();
            let answered: Value = call_point(&resolver, TOOLS, "call", &json!({"name": "match_facts", "arguments": {"equipes": "Villarreal - PSG"}, "context": {"userId": "u1", "messageId": "m9"}})).unwrap();
            let unknown: Value = call_point(&resolver, TOOLS, "call", &json!({"name": "nope", "arguments": {}, "context": {}})).unwrap();
            (listed, answered, unknown)
        })
        .await;

        assert_eq!(listed["tools"].as_array().unwrap().len(), 6);
        let facts = &answered["result"]["matchs"][0];
        assert_eq!(
            (
                facts["match"].as_str(),
                facts["statut"].as_str(),
                facts["source"].as_str()
            ),
            (
                Some("Villarreal – Paris Saint-Germain"),
                Some("en_cours"),
                Some("espn")
            )
        );
        assert!(facts["note"].as_str().unwrap().contains("décalées"));
        assert!(unknown["result"]["error"]
            .as_str()
            .unwrap()
            .contains("nope"));
    }

    #[test]
    fn the_lineup_tool_answers_with_rows_and_never_invents_unpublished_ones() {
        let rig = Rig::new("2026-11-03T20:11:00Z");
        let board = crate::runtime::tests::support::scoreboard_at(rig.now());
        for day in [
            crate::iso::day_key(rig.now() - 86_400.0),
            crate::iso::day_key(rig.now()),
            crate::iso::day_key(rig.now() + 86_400.0),
        ] {
            rig.espn.serve(&scoreboard("all", &day), &board);
        }
        rig.serve_summary(&villarreal_summary());

        let lineup = crate::tools::dispatch(&rig.sport, "compos_match", &json!({"equipes": "PSG"}));
        let nothing = crate::tools::dispatch(&rig.sport, "compos_match", &json!({}));
        let hint = crate::tools::dispatch(
            &rig.sport,
            "sport_channel_hint",
            &json!({"home": "A", "away": "B", "answer": "pas de chaine"}),
        );

        assert_eq!(
            (
                lineup["compos_publiees"].clone(),
                lineup["equipes"]["ext"]["formation"].clone()
            ),
            (json!(true), json!("4-3-3"))
        );
        assert!(nothing["error"]
            .as_str()
            .unwrap()
            .contains("ne reconnais pas"));
        assert_eq!(hint["ok"], false);
    }

    #[test]
    fn the_on_request_prono_answers_with_the_match_window_verdict_without_asking_for_a_role() {
        let rig = Rig::new("2026-11-03T20:11:00Z");
        let board = crate::runtime::tests::support::scoreboard_at(rig.now());
        for day in [
            crate::iso::day_key(rig.now() - 86_400.0),
            crate::iso::day_key(rig.now()),
            crate::iso::day_key(rig.now() + 86_400.0),
        ] {
            rig.espn.serve(&scoreboard("all", &day), &board);
        }

        let answer = crate::tools::dispatch(&rig.sport, "start_prono", &json!({"home": "Villarreal", "away": "PSG"}));

        assert_eq!(answer["ok"], json!(false));
        assert!(answer["error"].as_str().unwrap().contains("10e minute"));
        assert!(answer.get("raison").is_none());
    }
}
