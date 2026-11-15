use super::state::{in_window, Pronos};
use crate::detect::FlatMatch;
use crate::team_match::side_matches;

const ON_REQUEST_BEFORE_S: f64 = 6.0 * 3600.0;

pub fn find_for_prono<'a>(
    home: &str,
    away: &str,
    matches: &'a [FlatMatch],
    now: f64,
    pronos: &Pronos,
) -> Result<&'a FlatMatch, &'static str> {
    let found: Vec<&FlatMatch> = matches
        .iter()
        .filter(|m| {
            (side_matches(home, &m.home_names) && side_matches(away, &m.away_names))
                || (side_matches(home, &m.away_names) && side_matches(away, &m.home_names))
        })
        .collect();
    let [only] = found[..] else {
        return Err(if found.is_empty() {
            "matchIntrouvable"
        } else {
            "plusieursMatchs"
        });
    };
    if pronos.is_seen(&only.id) {
        return Err("dejaJoue");
    }
    if !in_window(only, now, ON_REQUEST_BEFORE_S) {
        return Err(if only.state != "pre" {
            "tropAvance"
        } else {
            "tropLointain"
        });
    }
    Ok(only)
}
