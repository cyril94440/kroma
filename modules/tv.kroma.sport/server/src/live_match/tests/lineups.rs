use super::support::{extracted, formation_summary, paris_monaco, villarreal, NOW_S};
use crate::live_match::{lineup_summary, revealed, MatchPayload};

fn row(m: &MatchPayload, side: &str, k: i64) -> Vec<String> {
    let mut players: Vec<_> = m
        .players
        .iter()
        .filter(|p| p.side == side && p.starter && p.row == Some(k))
        .collect();
    players.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
    players.into_iter().map(|p| p.position.clone()).collect()
}

type Placed = (String, Option<i64>, Option<i64>, Option<f64>);

fn placed(formation: &str, positions: &[&str]) -> Vec<Placed> {
    let m = extracted(&formation_summary(formation, positions));
    let mut starters: Vec<_> = m.players.iter().filter(|p| p.starter).collect();
    starters.sort_by_key(|p| (p.row.unwrap_or(-1), p.x.unwrap_or(0.0) as i64));
    starters
        .into_iter()
        .map(|p| (p.position.clone(), p.row, p.rows, p.x))
        .collect()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn the_server_places_a_442_and_a_433_from_the_villarreal_psg_fixture() {
    let m = extracted(&villarreal());

    assert_eq!(
        (
            m.teams.dom.formation.as_str(),
            m.teams.ext.formation.as_str()
        ),
        ("4-4-2", "4-3-3")
    );
    assert_eq!(row(&m, "dom", 0), ["G"]);
    assert_eq!(row(&m, "dom", 1), strings(&["LB", "CD-L", "CD-R", "RB"]));
    assert_eq!(row(&m, "dom", 2), strings(&["LM", "CM-L", "CM-R", "RM"]));
    assert_eq!(row(&m, "dom", 3), strings(&["CF-L", "CF-R"]));
    assert_eq!(row(&m, "ext", 1), strings(&["LB", "CD-L", "CD-R", "RB"]));
    assert_eq!(row(&m, "ext", 2), strings(&["LM", "CM", "RM"]));
    assert_eq!(row(&m, "ext", 3), strings(&["LF", "F", "RF"]));
    assert!(m
        .players
        .iter()
        .filter(|p| p.side == "dom" && p.starter)
        .all(|p| p.rows == Some(4)));
    assert!(m
        .players
        .iter()
        .filter(|p| !p.starter)
        .all(|p| p.row.is_none()));
    let x = |position: &str| {
        m.players
            .iter()
            .find(|p| p.side == "ext" && p.starter && p.position == position)
            .unwrap()
            .x
    };
    assert_eq!(
        (x("G"), x("LB"), x("RB"), x("F"), x("LF"), x("RF")),
        (
            Some(50.0),
            Some(20.0),
            Some(80.0),
            Some(50.0),
            Some(25.0),
            Some(75.0)
        )
    );
}

#[test]
fn a_4231_and_a_352_are_ordered_left_to_right_row_by_row() {
    let four = placed(
        "4-2-3-1",
        &[
            "G", "LB", "CD-L", "CD-R", "RB", "DM-L", "DM-R", "AM-L", "AM", "AM-R", "F",
        ],
    );
    let five = placed(
        "3-5-2",
        &[
            "G", "CD-L", "CD", "CD-R", "CM-L", "CM", "CM-R", "LWB", "RWB", "CF-L", "CF-R",
        ],
    );

    assert_eq!(
        four.iter().map(|p| (p.0.as_str(), p.1)).collect::<Vec<_>>(),
        [
            ("G", Some(0)),
            ("LB", Some(1)),
            ("CD-L", Some(1)),
            ("CD-R", Some(1)),
            ("RB", Some(1)),
            ("DM-L", Some(2)),
            ("DM-R", Some(2)),
            ("AM-L", Some(3)),
            ("AM", Some(3)),
            ("AM-R", Some(3)),
            ("F", Some(4))
        ]
    );
    assert!(four.iter().all(|p| p.2 == Some(5)));
    assert_eq!(
        five.iter()
            .filter(|p| p.1 == Some(2))
            .map(|p| p.0.as_str())
            .collect::<Vec<_>>(),
        ["LWB", "CM-L", "CM", "CM-R", "RWB"]
    );
    assert_eq!(
        five.iter()
            .filter(|p| p.1 == Some(2))
            .map(|p| p.3)
            .collect::<Vec<_>>(),
        [Some(16.7), Some(33.3), Some(50.0), Some(66.7), Some(83.3)]
    );
}

#[test]
fn an_unknown_position_falls_back_to_the_place_order_and_a_bad_formation_places_nothing() {
    let unknown = placed(
        "4-4-2",
        &[
            "G", "LB", "CD", "CD", "RB", "XX", "YY", "ZZ", "WW", "ST", "ST",
        ],
    );
    let mut eleven_not_starting = formation_summary(
        "4-4-2",
        &[
            "G", "CM", "CM", "CM", "CM", "CM", "CM", "CM", "CM", "CM", "CM",
        ],
    );
    eleven_not_starting["rosters"][0]["roster"][10]["starter"] = false.into();

    assert_eq!(
        unknown.iter().map(|p| p.1.unwrap()).collect::<Vec<_>>(),
        [0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3]
    );
    for formation in ["", "n/a", "4-4-3", "4-4-1-0"] {
        let positions: Vec<&str> = std::iter::once("G")
            .chain(std::iter::repeat_n("CM", 10))
            .collect();
        assert!(
            placed(formation, &positions)
                .iter()
                .all(|p| p.1.is_none() && p.2.is_none() && p.3.is_none()),
            "{formation}"
        );
    }
    assert!(extracted(&eleven_not_starting)
        .players
        .iter()
        .all(|p| p.row.is_none()));
}

#[test]
fn the_lineup_summary_gives_eleven_per_row_the_bench_and_only_revealed_changes() {
    let m = extracted(&villarreal());
    let summary = lineup_summary(&m, NOW_S, None);
    let psg = &summary.teams["ext"];

    assert!(
        summary.compos_publiees
            && summary.label.starts_with("Villarreal")
            && summary.changements_deja_faits.is_empty()
    );
    assert_eq!(
        (
            psg.formation.as_str(),
            psg.onze_par_ligne.iter().map(Vec::len).collect::<Vec<_>>()
        ),
        ("4-3-3", vec![1, 4, 3, 3])
    );
    assert_eq!(psg.onze_par_ligne[0], ["39 Safonov"]);
    assert!(psg.banc.len() >= 7);
    assert_eq!(
        psg.onze_par_ligne[1],
        strings(&["21 Hernández", "51 Pacho", "6 Zabarnyi", "2 Hakimi"])
    );
    assert_eq!(
        psg.onze_par_ligne[3],
        strings(&["14 Doué", "24 Mayulu", "11 Akliouche"])
    );
}

#[test]
fn unpublished_line_ups_are_said_so() {
    let mut summary = villarreal();
    summary["rosters"] = serde_json::json!([]);

    let lineup = lineup_summary(&extracted(&summary), NOW_S, None);

    assert!(
        !lineup.compos_publiees
            && lineup
                .note
                .as_deref()
                .is_some_and(|n| n.contains("pas encore publiées"))
    );
}

#[test]
fn substitutions_stay_hidden_until_the_delay_has_passed() {
    let mut m = extracted(&paris_monaco());

    let early = lineup_summary(&m, NOW_S + 30.0, None);
    let late = lineup_summary(&m, NOW_S + 61.0, None);
    let sub = m
        .facts
        .iter_mut()
        .find(|f| f.kind == "remplacement")
        .unwrap();
    let before = (
        revealed(sub.t_image, sub.t_vu, NOW_S * 1000.0 + 30_000.0, 60.0),
        revealed(sub.t_image, sub.t_vu, NOW_S * 1000.0 + 61_000.0, 60.0),
    );
    sub.t_image = Some(NOW_S as i64 * 1000 + 5000);
    let calibrated = (
        revealed(sub.t_image, sub.t_vu, NOW_S * 1000.0 + 6000.0, 600.0),
        revealed(sub.t_image, sub.t_vu, NOW_S * 1000.0 + 4000.0, 0.0),
    );

    assert!(early.changements_deja_faits.is_empty());
    assert!(
        late.changements_deja_faits.len() == 10 && !late.changements_deja_faits[0].entre.is_empty()
    );
    assert_eq!(before, (false, true));
    assert_eq!(calibrated, (true, false));
}
