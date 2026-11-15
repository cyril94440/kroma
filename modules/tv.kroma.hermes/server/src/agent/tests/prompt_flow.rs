use serde_json::json;

use crate::testing::{Rig, Scripted, Step};

fn model(rig: &Rig) -> Scripted {
    let model = Scripted::new((0..6).map(|_| Step::Calls(vec![("ignore", json!({}))])).collect());
    rig.use_model(std::sync::Arc::new(model.clone()));
    model
}

#[test]
fn the_house_rules_are_reloaded_at_each_message_and_close_the_system_prompt() {
    let rig = Rig::new();
    let model = model(&rig);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    rig.deps.rules.save("# Règles\n- Toujours tutoyer la famille.\n").unwrap();
    rig.hear("m-2", "Mamie", "mets m6");
    rig.drain();
    let opened = model.0.opened.lock().unwrap().clone();
    assert!(opened[0].0.contains("Chaînes FRANÇAISES en priorité"));
    assert!(opened[1].0.contains("Toujours tutoyer la famille") && !opened[1].0.contains("Chaînes FRANÇAISES en priorité"));
    assert!(opened[1].0.trim_end().ends_with("Toujours tutoyer la famille."));
    assert!(opened[1].0.contains("=== RÈGLES MAISON ==="));
}

#[test]
fn a_lesson_learned_appears_in_the_next_prompt() {
    let rig = Rig::new();
    let model = Scripted::new(vec![Step::Calls(vec![("learn_lesson", json!({ "lesson": "Quand on dit beinsport, c'est BEIN SPORTS 1." })), ("reply", json!({ "message": "Retenu." }))]), Step::Calls(vec![("ignore", json!({}))])]);
    rig.use_model(std::sync::Arc::new(model.clone()));
    rig.hear("m-1", "Mamie", "non, beinsport c'est la 1 ?");
    rig.drain();
    rig.hear("m-2", "Mamie", "mets beinsport");
    rig.drain();
    let opened = model.0.opened.lock().unwrap().clone();
    assert!(!opened[0].0.contains("c'est BEIN SPORTS 1."));
    assert!(opened[1].0.contains("## Leçons apprises (par Hermès)") && opened[1].0.contains("Quand on dit beinsport, c'est BEIN SPORTS 1."));
}

#[test]
fn the_context_carries_state_viewers_vote_poll_history_and_notes() {
    let rig = Rig::new();
    let model = model(&rig);
    rig.deps.store.add_note("u-mamie", "Mamie", "Supporter du RC Lens", rig.now(), 12).unwrap();
    rig.history_user("h-1", "Papi", "il est tard là");
    rig.agent.reconcile();
    rig.agent.on_event("party.vote", &json!({ "vote": { "label": "M6", "by": { "userId": "u-papi", "name": "Papi" }, "required": [{ "userId": "u-mamie", "name": "Mamie" }], "votes": {} } }));
    rig.agent.on_event("party.poll", &json!({ "poll": { "question": "Qui gagne ?", "kind": "choice", "options": ["Lens", "PSG"] } }));
    rig.control.status.lock().unwrap().health.black = true;
    rig.hear("m-1", "Mamie", "c'est quoi ce match ?");
    rig.drain();
    let first = model.0.opened.lock().unwrap()[0].1.clone();
    let lines: Vec<&str> = first.lines().collect();
    assert!(lines[0].starts_with("[état] Nous sommes le vendredi 6 novembre 2026, 22h20 (Paris)."), "{first}");
    for expected in [
        "[état] SONDAGE EN COURS : « Qui gagne ? » (type choice — options : Lens, PSG)",
        "[état] Chaîne en cours : TF1 (réf tf1) — en ce moment : Journal",
        "[état] Image du direct : NOIRE",
        "[état] Fournisseur IPTV actif : principal",
        "[état] Spectateurs : Mamie, Papi",
        "[état] VOTE EN COURS : M6 (demandé par Papi) — en attente de : Mamie.",
        "[ce que tu sais de Mamie]",
        "- Supporter du RC Lens",
        "[historique récent du chat]",
        "Papi: il est tard là",
        "[nouveau message à traiter]",
        "Mamie: c'est quoi ce match ?",
    ] {
        assert!(lines.contains(&expected) || first.contains(expected), "{expected}\n{first}");
    }
    let order = ["[état] Nous sommes", "[état] SONDAGE", "[état] Chaîne en cours", "[état] VOTE EN COURS", "[ce que tu sais", "[historique récent du chat]", "[nouveau message à traiter]"];
    let positions: Vec<usize> = order.iter().map(|k| first.find(k).unwrap()).collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
}

#[test]
fn the_history_never_repeats_the_message_being_answered_and_is_capped_at_twelve() {
    let rig = Rig::new();
    let model = model(&rig);
    for i in 0..20 {
        rig.history_user(&format!("h-{i}"), "Papi", &format!("ligne {i}"));
    }
    rig.history_user("m-1", "Mamie", "mets tf1");
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    let first = model.0.opened.lock().unwrap()[0].1.clone();
    assert_eq!(first.matches("Papi: ligne").count(), 12);
    assert_eq!(first.matches("mets tf1").count(), 1);
    assert!(first.contains("Papi: ligne 19") && !first.contains("Papi: ligne 7\n"));
}

#[test]
fn the_declared_tools_and_their_sections_follow_what_this_turn_can_do() {
    let rig = Rig::new();
    let model = model(&rig);
    rig.hear("m-1", "Mamie", "mets tf1");
    rig.drain();
    *model.0.no_search.lock().unwrap() = true;
    rig.hear("m-2", "Mamie", "mets m6");
    rig.drain();
    let opened = model.0.opened.lock().unwrap().clone();
    assert!(opened[0].2.contains(&"web_search".to_string()));
    assert!(!opened[1].2.contains(&"web_search".to_string()));
    assert!(opened[0].2.contains(&"look_at_live".to_string()));
    rig.deps.mark_unsupported("snapshot", rig.now());
    rig.hear("m-3", "Mamie", "mets arte");
    rig.drain();
    assert!(!model.0.opened.lock().unwrap()[2].2.contains(&"look_at_live".to_string()), "a live module without snapshot loses the tool");
    assert!(opened[0].0.contains("FAIT SUR UNE PERSONNE RÉELLE") && !opened[1].0.contains("FAIT SUR UNE PERSONNE RÉELLE"));
}

#[test]
fn the_sections_of_the_system_prompt_keep_a_stable_order() {
    let rig = Rig::new();
    let set = crate::tools::ToolSet::build(&rig.deps, &crate::tools::Availability { search: true, vision: true, ..crate::tools::Availability::default() }, rig.now());
    let prompt = crate::prompt::system_instruction(&set, "RULES", "fr", false);
    assert_eq!(
        prompt.sections,
        vec![
            "identity", "audience", "governance", "investigate", "failures", "references", "vision", "french", "context", "tools", "questions", "channel-requests", "ambiguity", "state-questions",
            "troubleshooting", "polls", "kick", "cards", "facts", "providers", "scans", "watch-party", "learning"
        ]
    );
    assert!(prompt.text.ends_with("=== RÈGLES MAISON ===\nRULES"));
    let bare = crate::tools::ToolSet::build(&rig.deps, &crate::tools::Availability::default(), rig.now());
    let without = crate::prompt::system_instruction(&bare, "RULES", "en", true);
    assert!(!without.sections.contains(&"facts") && without.sections.contains(&"extra-tools"));
    assert!(without.text.contains("Langue de tes réponses : anglais."));
}
