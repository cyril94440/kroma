use std::collections::BTreeMap;

use crate::prono::{clock_seconds, parse_score, points, rank};

fn votes(entries: &[(&str, &str)]) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    (
        entries
            .iter()
            .map(|(n, g)| (n.to_string(), g.to_string()))
            .collect(),
        entries
            .iter()
            .map(|(n, _)| (n.to_string(), n.to_string()))
            .collect(),
    )
}

#[test]
fn points_are_three_for_the_exact_score_one_for_the_winner_and_the_draw() {
    let earned: Vec<u32> = [
        ("0-3", "0-3"),
        ("0-2", "0-3"),
        ("1-1", "0-3"),
        ("2-2", "1-1"),
        ("3-0", "0-3"),
    ]
    .iter()
    .map(|(g, f)| points(g, f))
    .collect();

    assert_eq!(earned, [3, 1, 0, 1, 0]);
    assert_eq!(
        (parse_score("16-0"), parse_score("2-1"), parse_score("a-b")),
        (None, Some((2, 1)), None)
    );
}

#[test]
fn the_ranking_is_by_distance_then_by_winner() {
    let (v, n) = votes(&[("A", "0-2"), ("B", "2-0"), ("C", "1-1")]);
    let (v2, n2) = votes(&[("Cyril", "0-2"), ("Ced", "0-3"), ("Mamie", "1-1")]);

    let tied = rank(&v, &n, "1-0");
    let ranked = rank(&v2, &n2, "0-2");

    assert_eq!(
        tied.iter().map(|r| r.pseudo.as_str()).collect::<Vec<_>>(),
        ["B", "C", "A"]
    );
    assert_eq!(
        ranked
            .iter()
            .map(|r| (r.pseudo.as_str(), r.distance, r.texte.as_str()))
            .collect::<Vec<_>>(),
        [
            ("Cyril", 0, "en tête"),
            ("Ced", 1, "à 1 but"),
            ("Mamie", 2, "à 2 buts")
        ]
    );
}

#[test]
fn the_chrono_read_on_the_picture_is_minutes_and_seconds() {
    assert_eq!(
        (
            clock_seconds("12:34"),
            clock_seconds("45:10"),
            clock_seconds("0:05")
        ),
        (Some(754), Some(2710), Some(5))
    );
    assert_eq!(
        (
            clock_seconds("45'"),
            clock_seconds(""),
            clock_seconds("12:75")
        ),
        (None, None, None)
    );
}
