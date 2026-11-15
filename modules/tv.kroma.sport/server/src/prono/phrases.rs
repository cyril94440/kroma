use super::model::{Current, Status};

pub struct FinalText {
    pub key: &'static str,
    pub vars: Vec<(&'static str, String)>,
}

fn names(rows: impl Iterator<Item = String>) -> String {
    rows.collect::<Vec<_>>().join(", ")
}

pub fn final_text(current: &Current) -> Option<FinalText> {
    if current.status != Status::Final {
        return None;
    }
    let score = current.score.replace('-', "–");
    let base = |key, extra: Vec<(&'static str, String)>| {
        let mut vars = vec![
            ("home", current.home.clone()),
            ("away", current.away.clone()),
            ("score", score.clone()),
        ];
        vars.extend(extra);
        Some(FinalText { key, vars })
    };
    let exact: Vec<String> = current
        .results
        .iter()
        .filter(|r| r.exact)
        .map(|r| r.pseudo.clone())
        .collect();
    if !exact.is_empty() {
        return base(
            "sport.announce.prono.finalExact",
            vec![("names", names(exact.into_iter()))],
        );
    }
    let right: Vec<String> = current
        .results
        .iter()
        .filter(|r| r.points == 1)
        .map(|r| r.pseudo.clone())
        .collect();
    if !right.is_empty() {
        return base(
            "sport.announce.prono.finalWinner",
            vec![("names", names(right.into_iter()))],
        );
    }
    base(
        if current.votes.is_empty() {
            "sport.announce.prono.finalNobody"
        } else {
            "sport.announce.prono.finalNone"
        },
        Vec::new(),
    )
}

pub fn abandoned_text(current: &Current) -> FinalText {
    FinalText {
        key: "sport.announce.prono.cancelled",
        vars: vec![
            ("home", current.home.clone()),
            ("away", current.away.clone()),
        ],
    }
}
