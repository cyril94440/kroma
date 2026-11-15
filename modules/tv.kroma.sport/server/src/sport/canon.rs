use crate::text::norm;

pub const IMAGE_CONFIDENCE_MIN: f64 = 0.6;

const ALIASES: &[(&str, &str)] = &[
    ("psg", "psg"),
    ("paris sg", "psg"),
    ("paris saint germain", "psg"),
    ("paris st germain", "psg"),
    ("pfc", "paris fc"),
    ("paris fc", "paris fc"),
    ("rcl", "lens"),
    ("rc lens", "lens"),
    ("racing club de lens", "lens"),
    ("om", "marseille"),
    ("olympique de marseille", "marseille"),
    ("ol", "lyon"),
    ("olympique lyonnais", "lyon"),
    ("lyonnais", "lyon"),
    ("losc", "lille"),
    ("losc lille", "lille"),
    ("asm", "monaco"),
    ("as monaco", "monaco"),
    ("ogcn", "nice"),
    ("ogc nice", "nice"),
    ("srfc", "rennes"),
    ("stade rennais", "rennes"),
    ("rennais", "rennes"),
    ("fcn", "nantes"),
    ("fc nantes", "nantes"),
    ("rcsa", "strasbourg"),
    ("rc strasbourg", "strasbourg"),
    ("strasbourg alsace", "strasbourg"),
    ("tfc", "toulouse"),
    ("hac", "le havre"),
    ("lhac", "le havre"),
    ("havre", "le havre"),
    ("le havre ac", "le havre"),
    ("aja", "auxerre"),
    ("aj auxerre", "auxerre"),
    ("fcl", "lorient"),
    ("sco", "angers"),
    ("angers sco", "angers"),
    ("fcm", "metz"),
    ("fc metz", "metz"),
    ("sb29", "brest"),
    ("brestois", "brest"),
    ("stade brestois", "brest"),
    ("asse", "saint etienne"),
    ("as saint etienne", "saint etienne"),
    ("st etienne", "saint etienne"),
    ("sdr", "reims"),
    ("stade de reims", "reims"),
    ("inter", "inter"),
    ("inter milan", "inter"),
    ("internazionale", "inter"),
    ("juve", "juventus"),
    ("barca", "barcelone"),
    ("barcelona", "barcelone"),
    ("fc barcelone", "barcelone"),
    ("real", "real madrid"),
    ("rma", "real madrid"),
    ("bayern munich", "bayern"),
    ("bayern munchen", "bayern"),
    ("fc bayern", "bayern"),
    ("bayern", "bayern"),
    ("bvb", "dortmund"),
    ("borussia dortmund", "dortmund"),
    ("man city", "manchester city"),
    ("man utd", "manchester united"),
    ("man united", "manchester united"),
    ("atletico", "atletico madrid"),
    ("atletico de madrid", "atletico madrid"),
    ("atleti", "atletico madrid"),
];

pub const GENERIC: &[&str] = &[
    "fc",
    "ac",
    "as",
    "sc",
    "rc",
    "ogc",
    "us",
    "afc",
    "cf",
    "club",
    "football",
    "stade",
    "olympique",
    "racing",
    "de",
    "du",
    "la",
    "le",
    "les",
    "l",
    "d",
    "sporting",
];

fn alias(text: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|(from, _)| *from == text)
        .map(|(_, to)| *to)
}

pub fn canon_team(raw: &str) -> Option<String> {
    let folded = norm(raw);
    if folded.is_empty() {
        return None;
    }
    if let Some(known) = alias(&folded) {
        return Some(known.to_string());
    }
    let words: Vec<&str> = folded.split(' ').filter(|w| !GENERIC.contains(w)).collect();
    let stripped = if words.is_empty() {
        folded.clone()
    } else {
        words.join(" ")
    };
    let resolved = alias(&stripped).map_or(stripped, str::to_string);
    (resolved != "paris").then_some(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_names_and_initials_to_one_club() {
        assert_eq!(canon_team("Olympique Lyonnais").as_deref(), Some("lyon"));
        assert_eq!(canon_team("PSG").as_deref(), Some("psg"));
        assert_eq!(canon_team("Paris FC").as_deref(), Some("paris fc"));
    }

    #[test]
    fn a_bare_paris_is_ambiguous() {
        assert_eq!(canon_team("Paris"), None);
        assert_eq!(canon_team(""), None);
    }
}
