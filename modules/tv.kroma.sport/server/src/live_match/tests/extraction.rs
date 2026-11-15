use serde_json::json;

use super::support::{extracted, paris_monaco, play_mut, NOW_MS};

#[test]
fn header_teams_and_state_come_from_the_summary() {
    let m = extracted(&paris_monaco());

    assert_eq!(
        (
            m.id.as_str(),
            m.state.as_str(),
            m.channel.as_str(),
            m.competition.as_str()
        ),
        ("espn-401876415", "post", "FR: LIGUE1+ 5", "Ligue 1")
    );
    assert_eq!(
        (
            m.teams.dom.name.as_str(),
            m.teams.dom.abbr.as_str(),
            m.teams.dom.color.as_str(),
            m.teams.dom.formation.as_str()
        ),
        ("Paris FC", "PAR", "#000066", "4-2-3-1")
    );
    assert_eq!(
        (m.teams.ext.name.as_str(), m.teams.ext.color.as_str()),
        ("AS Monaco", "#d71431")
    );
    assert_eq!((m.delay_s, m.updated_ms), (60.0, NOW_MS));
}

#[test]
fn starters_come_before_the_bench_and_the_server_places_them() {
    let m = extracted(&paris_monaco());

    let starters = m.players.iter().filter(|p| p.starter).count();
    let trapp = m.players.iter().find(|p| p.name == "Trapp").unwrap();

    assert_eq!((starters, m.players.len()), (22, 40));
    assert!(
        m.players[..20].iter().all(|p| p.side == "dom")
            && m.players[..11].iter().all(|p| p.starter)
    );
    assert_eq!(
        (
            trapp.id.as_str(),
            trapp.num,
            trapp.position.as_str(),
            trapp.place,
            trapp.row,
            trapp.rows,
            trapp.x
        ),
        ("122280", 1, "G", 1, Some(0), Some(5), Some(50.0))
    );
    assert!(
        m.players.iter().any(|p| p.name == "Lees-Melou")
            && m.players.iter().any(|p| p.name == "López")
    );
}

#[test]
fn facts_are_typed_and_counted_as_the_source_does() {
    let m = extracted(&paris_monaco());
    let count = |kind: &str, side: &str| {
        m.facts
            .iter()
            .filter(|f| f.kind == kind && f.side == side)
            .count()
    };

    assert_eq!((count("but", "dom"), count("but", "ext")), (3, 1));
    assert_eq!(
        m.facts.iter().filter(|f| f.kind == "remplacement").count(),
        10
    );
    assert_eq!(
        (
            m.facts.iter().filter(|f| f.kind == "jaune").count(),
            m.facts.iter().filter(|f| f.kind == "rouge").count()
        ),
        (4, 0)
    );
    assert_eq!((count("corner", "dom"), count("corner", "ext")), (3, 2));
    assert_eq!(count("tir_contre", "dom") + count("tir_contre", "ext"), 4);
    assert_eq!(count("tir_cadre", "dom") + count("tir_cadre", "ext"), 8);
}

#[test]
fn a_goal_carries_its_scorer_and_assist() {
    let m = extracted(&paris_monaco());

    let goal = m.facts.iter().find(|f| f.id == "54659513").unwrap();
    let id_of = |name: &str| {
        m.players
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id.clone())
    };

    assert_eq!(
        (
            goal.kind.as_str(),
            goal.side.as_str(),
            goal.minute.as_str(),
            goal.s
        ),
        ("but", "dom", "38'", 2250)
    );
    assert_eq!(
        (goal.player.clone(), goal.assist.clone()),
        (id_of("Sinayoko"), id_of("López"))
    );
    assert_eq!((goal.x, goal.y), (Some(92.3), Some(47.0)));
}

#[test]
fn a_substitution_names_who_came_on_and_who_went_off() {
    let m = extracted(&paris_monaco());

    let sub = m.facts.iter().find(|f| f.id == "54664259").unwrap();
    let id_of = |name: &str| {
        m.players
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id.clone())
    };

    assert_eq!(
        (
            sub.kind.as_str(),
            sub.side.as_str(),
            sub.came_on.clone(),
            sub.went_off.clone(),
            sub.x
        ),
        (
            "remplacement",
            "ext",
            id_of("Detourbet"),
            id_of("Názinho"),
            None
        )
    );
}

#[test]
fn a_pre_match_summary_and_an_empty_one_do_not_break() {
    let mut pre = paris_monaco();
    pre["header"]["competitions"][0]["status"]["type"]["state"] = "pre".into();
    pre["commentary"] = json!([]);
    pre["boxscore"] = json!({});

    let m = extracted(&pre);
    let empty = extracted(&json!({}));

    assert_eq!(
        (
            m.state.as_str(),
            m.facts.len(),
            m.possession.len(),
            m.players.len()
        ),
        ("pre", 0, 0, 40)
    );
    assert!(empty.players.is_empty());
}

#[test]
fn an_own_goal_benefits_the_opponent_and_has_no_assist() {
    let mut summary = paris_monaco();
    play_mut(&mut summary, "54659513")["type"]["text"] = "Own Goal".into();

    let m = extracted(&summary);

    let goal = m.facts.iter().find(|f| f.id == "54659513").unwrap();
    assert_eq!(
        (goal.kind.as_str(), goal.side.as_str(), goal.assist.clone()),
        ("but", "ext", None)
    );
}

#[test]
fn every_espn_goal_variant_is_a_goal_with_an_assist() {
    for label in ["Goal - Header", "Goal - Volley", "Goal - Free-kick"] {
        let mut summary = paris_monaco();
        play_mut(&mut summary, "54659513")["type"]["text"] = label.into();

        let m = extracted(&summary);

        let goal = m.facts.iter().find(|f| f.id == "54659513").unwrap();
        assert_eq!(
            m.facts.iter().filter(|f| f.kind == "but").count(),
            4,
            "{label}"
        );
        assert!(goal.player.is_some() && goal.assist.is_some(), "{label}");
    }
    for label in [
        "Shot On Target",
        "Goal Kick",
        "Penalty - Saved",
        "Substitution",
    ] {
        assert_ne!(
            crate::live_match::contract_type_of(label),
            Some("but"),
            "{label}"
        );
    }
}
