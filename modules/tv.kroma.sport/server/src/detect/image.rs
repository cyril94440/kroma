use std::sync::LazyLock;

use regex::Regex;

use super::competitions::slug_competition;
use super::flat::FlatMatch;
use super::names::name_designates;
use crate::sport::Reading;
use crate::text::norm;

pub const IMAGE_CONFIDENCE_MIN: f64 = 0.6;

static SCORE_READ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{1,2})\s*[-–:]\s*(\d{1,2})").unwrap());
static SCORE_ESPN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)-(\d+)$").unwrap());

pub fn score_compatible(read: &str, found: &FlatMatch) -> Option<bool> {
    let m = SCORE_READ.captures(read)?;
    let e = SCORE_ESPN.captures(&found.score)?;
    if found.state == "pre" {
        return None;
    }
    let n = |c: &regex::Captures, i: usize| c[i].parse::<i64>().unwrap_or(0);
    let (a, b, h, x) = (n(&m, 1), n(&m, 2), n(&e, 1), n(&e, 2));
    Some((a <= h && b <= x) || (a <= x && b <= h))
}

fn others_forms(
    found: &FlatMatch,
    home: bool,
    candidates: &[FlatMatch],
) -> Vec<super::names::Forms> {
    let mut out = vec![found.forms(!home, true)];
    for other in candidates.iter().filter(|c| c.id != found.id) {
        out.push(other.forms(true, true));
        out.push(other.forms(false, true));
    }
    out
}

fn sigil_designates(sigil: &str, found: &FlatMatch, home: bool, candidates: &[FlatMatch]) -> bool {
    name_designates(sigil, &found.forms(home, true), &[])
        && !others_forms(found, home, candidates)
            .iter()
            .any(|f| name_designates(sigil, f, &[]))
}

pub fn read_image(
    found: &FlatMatch,
    image: Option<&Reading>,
    candidates: &[FlatMatch],
) -> (f64, Vec<String>) {
    let Some(image) = image.filter(|i| {
        i.kind.trim().eq_ignore_ascii_case("direct") && i.confidence >= IMAGE_CONFIDENCE_MIN
    }) else {
        return (0.0, Vec::new());
    };
    let (mut sides, mut names_read) = ([false, false], 0);
    for (name, sigil) in [(&image.team1, &image.sigil1), (&image.team2, &image.sigil2)] {
        let sigil = sigil.trim();
        let name = if !name.trim().is_empty() && norm(name) == norm(sigil) {
            ""
        } else {
            name.trim()
        };
        names_read += usize::from(!name.is_empty());
        for (index, home) in [true, false].into_iter().enumerate() {
            let by_name = !name.is_empty()
                && name_designates(
                    name,
                    &found.forms(home, true),
                    &others_forms(found, home, candidates),
                );
            let by_sigil = !sigil.is_empty() && sigil_designates(sigil, found, home, candidates);
            sides[index] |= by_name || by_sigil;
        }
    }
    let (mut points, mut reasons) = (0.0, Vec::new());
    match sides.iter().filter(|s| **s).count() {
        2 => {
            points += 12.0;
            reasons.push("les deux équipes lues à l'image".to_string());
        }
        1 => {
            points += 5.0;
            reasons.push("une équipe lue à l'image".to_string());
        }
        _ if names_read >= 2 => {
            points -= 3.0;
            reasons.push("d'autres équipes lues à l'image".to_string());
        }
        _ => {}
    }
    if slug_competition(&image.competition)
        .zip(found.slug())
        .is_some_and(|(a, b)| a == b)
    {
        points += 1.0;
    }
    if sides.iter().any(|s| *s) {
        match score_compatible(&image.score, found) {
            Some(true) => {
                points += 2.0;
                reasons.push("score lu à l'image cohérent".to_string());
            }
            Some(false) => {
                points -= 3.0;
                reasons.push("score lu à l'image incohérent".to_string());
            }
            None => {}
        }
    }
    (points, reasons)
}
