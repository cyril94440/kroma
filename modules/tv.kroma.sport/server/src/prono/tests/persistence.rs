use super::support::{havre, matches, pronos, t0};
use crate::prono::{File, Pronos, KEEP_FINAL_S};

fn restart(p: &Pronos) -> Pronos {
    let text = serde_json::to_string(&p.to_file()).unwrap();
    Pronos::from_file(serde_json::from_str(&text).unwrap(), 60.0, 15.0)
}

#[test]
fn the_season_and_the_prono_in_play_survive_a_restart() {
    let mut p = pronos();
    let opening = matches(&[havre()]);
    p.cycle(
        Some(&opening),
        opening.first(),
        "c",
        &[("u-a".into(), "A".into()), ("u-b".into(), "B".into())],
        t0(),
    );
    p.vote("u-a", "A", "espn-401", "0-3", t0() + 5.0);

    let mut second = restart(&p);
    let whistle = matches(&[havre().at("post", "90'", 2).scored("0", "3")]);
    second.follow(whistle.first(), true, t0() + 7000.0);
    let mut third = restart(&second);
    let kept = third.current.as_ref().map(|c| c.status);
    third.purge(t0() + 7000.0 + KEEP_FINAL_S + 1.0);
    let fourth = restart(&third);

    assert_eq!(
        second
            .current
            .as_ref()
            .unwrap()
            .votes
            .get("u-a")
            .map(String::as_str),
        Some("0-3")
    );
    assert!(second.is_seen("espn-401"));
    assert_eq!(third.season["u-a"].points, 3);
    assert_eq!(kept, Some(crate::prono::Status::Final));
    assert!(fourth.current.is_none() && fourth.season["u-a"].points == 3);
}

#[test]
fn a_corrupt_or_unexpected_file_reads_as_an_empty_state() {
    for text in [
        "{not json",
        "[]",
        r#"{"seen": 5, "season": {"A": "x"}}"#,
        "",
    ] {
        let file: File = serde_json::from_str(text).unwrap_or_default();

        assert!(
            file.season.is_empty() && file.current.is_none() && file.seen.is_empty(),
            "{text}"
        );
    }
}
