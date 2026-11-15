use super::support::Rig;

#[test]
fn a_prono_opens_on_the_detected_match_is_placed_and_scored_at_the_whistle() {
    let rig = Rig::new("2026-11-03T19:40:00Z");
    rig.serve_summary(&super::support::villarreal_summary());

    let opened = rig.run_until(120, |r| r.sport.prono_payload().is_some());
    let opening = rig.room.texts();
    let payload = rig.sport.prono_payload().unwrap();
    let id = payload["id"].as_str().unwrap().to_string();
    let placed = [
        ("u-ana", "Ana", "0-1"),
        ("u-ced", "Ced", "1-0"),
        ("u-mamie", "Mamie", "0-2"),
    ]
    .map(|(u, n, s)| rig.sport.vote(u, n, &id, s));
    let bad = rig.sport.vote("u-ana", "Ana", &id, "99-0");
    rig.run_coarse(1800, 20);
    let closed = rig.sport.vote("u-ana", "Ana", &id, "2-2");
    rig.run_coarse(7500, 60);
    let finished = rig.run_until(400, |r| {
        r.sport
            .prono_payload()
            .is_some_and(|p| p["statut"] == "final")
    });

    let result = rig.sport.prono_payload().unwrap();
    assert!(opened);
    assert!(
        opening[0].starts_with("À vos pronos ! Villarreal – Paris Saint-Germain"),
        "{}",
        opening[0]
    );
    assert_eq!(
        (payload["statut"].clone(), payload["auto"].clone()),
        ("ouvert".into(), true.into())
    );
    assert!(placed.iter().all(Result::is_ok));
    assert_eq!(bad.unwrap_err().code(), "invalidScore");
    assert_eq!(closed.unwrap_err().code(), "closed");
    assert!(finished);
    let rows: Vec<(String, u64)> = result["resultats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["pseudo"].as_str().unwrap().to_string(),
                r["points"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Ana".to_string(), 3),
            ("Mamie".to_string(), 1),
            ("Ced".to_string(), 0)
        ]
    );
    assert!(
        rig.room
            .texts()
            .iter()
            .any(|t| t.contains("Score exact pour Ana (+3)")),
        "{:?}",
        rig.room.texts()
    );
    let board = rig.sport.leaderboard();
    assert_eq!(
        (board[0]["pseudo"].clone(), board[0]["points"].clone()),
        ("Ana".into(), 3.into())
    );
}
