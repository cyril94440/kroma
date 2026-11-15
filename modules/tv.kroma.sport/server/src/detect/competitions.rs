use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::text::{contains_words, norm};

const COMPETITIONS: &[(&str, &[&str])] = &[
    (
        "uefa.champions",
        &[
            "uefa champions league",
            "champions league",
            "ligue des champions",
            "ligue des champions de l uefa",
            "ldc",
            "c1",
        ],
    ),
    (
        "uefa.europa",
        &["uefa europa league", "europa league", "ligue europa"],
    ),
    (
        "uefa.europa.conf",
        &[
            "uefa europa conference league",
            "uefa conference league",
            "conference league",
            "ligue conference",
            "europa conference league",
            "ligue europa conference",
        ],
    ),
    (
        "fra.1",
        &[
            "french ligue 1",
            "ligue 1",
            "ligue 1 uber eats",
            "ligue 1 mcdonald s",
            "ligue 1 mcdonalds",
        ],
    ),
    ("fra.2", &["french ligue 2", "ligue 2", "ligue 2 bkt"]),
    ("fra.coupe_de_france", &["coupe de france", "french cup"]),
    ("eng.1", &["english premier league", "premier league"]),
    ("eng.fa", &["fa cup", "emirates fa cup"]),
    (
        "eng.league_cup",
        &["english league cup", "carabao cup", "efl cup"],
    ),
    (
        "esp.1",
        &[
            "spanish laliga",
            "laliga",
            "la liga",
            "liga espagnole",
            "championnat d espagne",
        ],
    ),
    ("esp.copa_del_rey", &["copa del rey", "coupe du roi"]),
    ("ita.1", &["italian serie a", "serie a"]),
    ("ita.coppa_italia", &["coppa italia", "coupe d italie"]),
    ("ger.1", &["german bundesliga", "bundesliga"]),
    ("ger.dfb_pokal", &["dfb pokal", "coupe d allemagne"]),
    ("fifa.world", &["fifa world cup", "coupe du monde"]),
    (
        "fifa.worldq.uefa",
        &[
            "world cup qualifying uefa",
            "qualifications coupe du monde",
            "eliminatoires coupe du monde",
        ],
    ),
    (
        "uefa.nations",
        &["uefa nations league", "nations league", "ligue des nations"],
    ),
    (
        "uefa.euro",
        &[
            "uefa euro",
            "european championship",
            "championnat d europe",
            "euro 2028",
        ],
    ),
    (
        "fifa.friendly",
        &[
            "international friendly",
            "match amical international",
            "amical international",
        ],
    ),
    (
        "club.friendly",
        &["club friendly", "match amical", "amical", "friendly"],
    ),
];

const ESPN_NAMES: &[(&str, &str)] = &[
    ("uefa champions league", "uefa.champions"),
    ("uefa europa league", "uefa.europa"),
    ("uefa conference league", "uefa.europa.conf"),
    ("uefa europa conference league", "uefa.europa.conf"),
    ("ligue 1", "fra.1"),
    ("french ligue 1", "fra.1"),
    ("ligue 2", "fra.2"),
    ("french ligue 2", "fra.2"),
    ("coupe de france", "fra.coupe_de_france"),
    ("french cup", "fra.coupe_de_france"),
    ("premier league", "eng.1"),
    ("english premier league", "eng.1"),
    ("fa cup", "eng.fa"),
    ("emirates fa cup", "eng.fa"),
    ("carabao cup", "eng.league_cup"),
    ("english league cup", "eng.league_cup"),
    ("laliga", "esp.1"),
    ("spanish laliga", "esp.1"),
    ("copa del rey", "esp.copa_del_rey"),
    ("serie a", "ita.1"),
    ("italian serie a", "ita.1"),
    ("coppa italia", "ita.coppa_italia"),
    ("bundesliga", "ger.1"),
    ("german bundesliga", "ger.1"),
    ("dfb pokal", "ger.dfb_pokal"),
    ("fifa world cup", "fifa.world"),
    ("uefa nations league", "uefa.nations"),
    ("uefa euro", "uefa.euro"),
    ("uefa european championship", "uefa.euro"),
    ("international friendly", "fifa.friendly"),
    ("club friendly", "club.friendly"),
];

const NON_UEFA_FEDERATIONS: [&str; 7] =
    ["afc", "caf", "concacaf", "conmebol", "ofc", "ncaa", "ncaam"];

static FEMININE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(women|womens|woman|ladies|feminin|feminine|femenino|femminile|frauen|femmes?|girls|ncaaw|nwsl|wsl|arkema|liga f)\b").unwrap()
});
static YOUTH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(u ?(1[5-9]|2[0-3])|under ?(1[5-9]|2[0-3])|reserves?|reservas?|youth|juniors?|espoirs|jeunes|academy|primavera|ii|b team)\b").unwrap()
});
static TEAM_SUFFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s(B|II|W)$").unwrap());

pub fn slug_competition(name: &str) -> Option<&'static str> {
    let n = norm(name);
    if n.is_empty()
        || NON_UEFA_FEDERATIONS.contains(&n.split(' ').next().unwrap_or_default())
        || n.starts_with("ncaaw")
    {
        return None;
    }
    let mut best: Option<(&'static str, usize)> = None;
    for (slug, names) in COMPETITIONS {
        for x in names.iter().filter(|x| contains_words(&n, x)) {
            if best.is_none_or(|(_, len)| x.len() > len) {
                best = Some((slug, x.len()));
            }
        }
    }
    best.map(|(slug, _)| slug)
}

pub fn slug_espn(name: &str) -> Option<&'static str> {
    let head = name.split(',').next().unwrap_or_default();
    let n = norm(head);
    ESPN_NAMES
        .iter()
        .find(|(k, _)| *k == n)
        .map(|(_, slug)| *slug)
}

pub fn competitions_cited(text: &str) -> BTreeSet<&'static str> {
    COMPETITIONS
        .iter()
        .filter(|(_, names)| {
            names
                .iter()
                .any(|x| (x.len() > 3 || *x == "c1") && contains_words(text, x))
        })
        .map(|(slug, _)| *slug)
        .collect()
}

pub fn is_special(competition: &str, home: &str, away: &str) -> bool {
    let joined = norm(&format!("{competition} {home} {away}"));
    FEMININE.is_match(&joined)
        || YOUTH.is_match(&joined)
        || [home, away].iter().any(|m| TEAM_SUFFIX.is_match(m))
}

pub fn special_cited(text: &str) -> bool {
    FEMININE.is_match(text) || YOUTH.is_match(text)
}
