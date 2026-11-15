use super::support::{villarreal_summary, Rig};

#[test]
fn the_match_on_the_current_channel_is_detected_and_its_panel_published() {
    let rig = Rig::new("2026-11-03T20:11:00Z");
    rig.serve_summary(&villarreal_summary());

    let found = rig.run_until(90, |r| !r.events("sport.match").is_empty());

    let panel = rig.events("sport.match");
    let first = &panel[0]["match"];
    assert!(found);
    assert_eq!(
        (
            first["id"].as_str(),
            first["equipes"]["dom"]["nom"].as_str(),
            first["equipes"]["ext"]["nom"].as_str()
        ),
        (
            Some("espn-401915309"),
            Some("Villarreal"),
            Some("Paris Saint-Germain")
        )
    );
    assert_eq!(first["chaine"], "FR: Canal+ Foot");
    assert!(first["joueurs"].as_array().unwrap().len() > 22);
    assert_eq!(
        first["evenements"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == "but")
            .count(),
        1
    );
    let snapshot = rig.sport.snapshot("u-ana", "Ana", false);
    assert_eq!(snapshot["match"]["id"], "espn-401915309");
    assert!(snapshot["seq"].as_u64().unwrap() >= panel[0]["seq"].as_u64().unwrap());
}

#[test]
fn the_events_carry_a_growing_sequence_and_the_snapshot_never_falls_behind() {
    let rig = Rig::new("2026-11-03T20:11:00Z");
    rig.serve_summary(&villarreal_summary());

    rig.run(70);

    let seqs: Vec<u64> = rig
        .sport
        .tap
        .lock()
        .unwrap()
        .iter()
        .map(|(_, p)| p["seq"].as_u64().unwrap())
        .collect();
    assert!(seqs.len() >= 2 && seqs.windows(2).all(|w| w[0] < w[1]));
    assert!(
        rig.sport.snapshot("u-ana", "Ana", false)["seq"]
            .as_u64()
            .unwrap()
            >= *seqs.last().unwrap()
    );
}

#[test]
fn nothing_is_followed_when_nobody_watches() {
    let rig = Rig::new("2026-11-03T20:11:00Z");
    rig.watch_canal_foot(&[]);
    rig.serve_summary(&villarreal_summary());

    rig.run(60);

    assert!(rig.events("sport.match").is_empty());
    assert!(!rig.espn.calls().iter().any(|c| c.contains("summary")));
}
