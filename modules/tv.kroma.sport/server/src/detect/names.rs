use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use crate::text::{contains_words, norm, norm_lossy};

pub type Forms = BTreeSet<String>;

const EMPTY_WORDS: &[&str] = &[
    "fc", "cf", "sc", "ac", "as", "rc", "afc", "ssc", "sv", "fk", "sk", "cd", "ud", "ca", "ogc",
    "us", "club", "de", "du", "la", "le", "les", "of", "the", "and", "et", "d", "1", "calcio",
    "football",
];

const AMBIGUOUS_WORDS: &[&str] = &[
    "paris",
    "united",
    "city",
    "real",
    "athletic",
    "sporting",
    "olympique",
    "racing",
    "saint",
    "st",
    "rovers",
    "town",
    "county",
    "albion",
    "rangers",
    "wanderers",
    "hotspur",
    "north",
    "south",
    "east",
    "west",
    "new",
    "royal",
    "nacional",
    "deportivo",
    "atletico",
    "independiente",
    "internacional",
    "dinamo",
    "dynamo",
    "spartak",
    "lokomotiv",
    "borussia",
    "olympic",
    "victoria",
    "universitario",
    "juniors",
    "young",
    "boys",
    "red",
    "star",
    "union",
    "academy",
];

const GROUPS: &[&[&str]] = &[
    &[
        "paris saint germain",
        "psg",
        "paris sg",
        "paris st germain",
        "paris saint germain fc",
    ],
    &[
        "marseille",
        "olympique de marseille",
        "om",
        "olympique marseille",
    ],
    &["lyon", "olympique lyonnais", "ol", "lyonnais"],
    &["lille", "losc", "losc lille", "lille osc"],
    &["monaco", "as monaco", "asm"],
    &["lens", "rc lens", "rcl"],
    &["rennes", "stade rennais", "rennais", "srfc"],
    &["nice", "ogc nice", "ogcn"],
    &["strasbourg", "rc strasbourg", "rcsa"],
    &["nantes", "fc nantes", "fcn"],
    &["brest", "stade brestois", "brestois", "sb29"],
    &["le havre", "havre", "hac", "lhac", "le havre ac"],
    &["toulouse", "toulouse fc", "tfc"],
    &["auxerre", "aj auxerre", "aja"],
    &["lorient", "fc lorient", "fcl"],
    &["angers", "angers sco", "sco"],
    &["metz", "fc metz", "fcm"],
    &["saint etienne", "as saint etienne", "st etienne", "asse"],
    &["reims", "stade de reims", "sdr"],
    &["paris fc", "pfc"],
    &[
        "barcelona",
        "barcelone",
        "fc barcelone",
        "fc barcelona",
        "barca",
    ],
    &["real madrid", "real madrid cf"],
    &[
        "atletico madrid",
        "atletico de madrid",
        "atleti",
        "atletico",
    ],
    &[
        "bayern munich",
        "bayern munchen",
        "bayern",
        "fc bayern",
        "bayern munich fc",
    ],
    &[
        "inter",
        "internazionale",
        "inter milan",
        "internazionale milano",
        "inter milano",
    ],
    &["ac milan", "milan", "milan ac"],
    &["juventus", "juve"],
    &["napoli", "naples", "ssc napoli"],
    &["roma", "as roma", "rome", "rom"],
    &[
        "manchester united",
        "manchester utd",
        "man utd",
        "man united",
        "mun",
    ],
    &["manchester city", "man city"],
    &["tottenham hotspur", "tottenham", "spurs"],
    &["newcastle united", "newcastle"],
    &["sevilla", "seville", "fc seville"],
    &["athletic club", "athletic bilbao", "bilbao"],
    &["borussia dortmund", "dortmund", "bvb"],
    &["bayer leverkusen", "leverkusen", "bayer 04 leverkusen"],
    &["eintracht frankfurt", "francfort", "frankfurt"],
    &["rb leipzig", "leipzig"],
    &[
        "shakhtar donetsk",
        "chakhtior donetsk",
        "shakhtar",
        "chakhtior",
    ],
    &[
        "sporting cp",
        "sporting club portugal",
        "sporting lisbonne",
        "sporting lisbon",
    ],
    &["club brugge", "bruges", "brugge", "club bruges"],
    &["copenhagen", "copenhague", "fc copenhague", "fc copenhagen"],
    &["slavia prague", "slavia praha", "sk slavia prague"],
    &["bodo glimt", "bodo/glimt", "fk bodo glimt", "bod glimt"],
    &["psv eindhoven", "psv"],
    &["galatasaray", "galatasaray sk"],
    &["olympiacos", "olympiakos"],
    &[
        "union saint gilloise",
        "union sg",
        "union st gilloise",
        "royale union saint gilloise",
    ],
    &["vfb stuttgart", "stuttgart"],
    &["feyenoord", "feyenoord rotterdam"],
    &["qarabag", "qarabag fk"],
    &["kairat almaty", "kairat"],
    &["pafos", "pafos fc"],
    &["villarreal", "villarreal cf"],
];

static GROUP_INDEX: LazyLock<BTreeMap<String, Forms>> = LazyLock::new(|| {
    let mut index: BTreeMap<String, Forms> = BTreeMap::new();
    for group in GROUPS {
        let forms: Forms = group
            .iter()
            .flat_map(|x| [without_empty(&norm(x)), norm(x)])
            .collect();
        for form in &forms {
            index
                .entry(form.clone())
                .or_default()
                .extend(forms.iter().cloned());
        }
    }
    index
});

fn is_ambiguous(word: &str) -> bool {
    AMBIGUOUS_WORDS.contains(&word)
}

pub fn without_empty(text: &str) -> String {
    let kept: Vec<&str> = text
        .split(' ')
        .filter(|w| !EMPTY_WORDS.contains(w))
        .collect();
    if kept.is_empty() {
        text.to_string()
    } else {
        kept.join(" ")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamNames {
    pub display: String,
    pub short: String,
    pub name: String,
    pub location: String,
    pub abbreviation: String,
}

pub fn team_forms(team: &TeamNames, with_abbreviation: bool) -> Forms {
    let raw: Vec<&String> = [&team.display, &team.short, &team.name, &team.location]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let whole: Forms = [&team.display, &team.short]
        .into_iter()
        .filter(|s| !s.is_empty())
        .flat_map(|s| [norm(s), norm_lossy(s)])
        .collect();
    let mut forms = Forms::new();
    for name in raw {
        for normalised in [norm(name), norm_lossy(name)] {
            if normalised.is_empty() {
                continue;
            }
            forms.insert(without_empty(&normalised));
            forms.insert(normalised);
        }
    }
    let grouped: Forms = forms
        .iter()
        .filter_map(|f| GROUP_INDEX.get(f))
        .flatten()
        .cloned()
        .collect();
    forms.extend(grouped);
    if with_abbreviation && !team.abbreviation.is_empty() {
        let sigil = norm(&team.abbreviation);
        if !sigil.is_empty() {
            forms.insert(sigil);
        }
    }
    forms
        .into_iter()
        .filter(|f| {
            !f.is_empty()
                && (!is_ambiguous(f) || whole.contains(f))
                && !f.chars().all(|c| c.is_ascii_digit())
                && (f.len() >= 3 || GROUP_INDEX.contains_key(f))
        })
        .collect()
}

fn keys(name: &str) -> Forms {
    let stripped = without_empty(name);
    let mut keys: Forms = [name.to_string(), stripped.clone()].into_iter().collect();
    for k in [name, stripped.as_str()] {
        if let Some(group) = GROUP_INDEX.get(k) {
            keys.extend(group.iter().cloned());
        }
    }
    keys
}

fn approximates(name: &str, forms: &Forms) -> bool {
    let candidates: Forms = [name.to_string(), without_empty(name)]
        .into_iter()
        .filter(|c| !c.is_empty() && !is_ambiguous(c))
        .collect();
    candidates.iter().any(|c| {
        c.len() >= 5
            && (forms.iter().any(|f| contains_words(f, c))
                || forms.iter().any(|f| f.len() >= 5 && contains_words(c, f)))
    })
}

fn intersects(a: &Forms, b: &Forms) -> bool {
    a.iter().any(|x| b.contains(x))
}

pub fn name_designates(name: &str, forms: &Forms, others: &[Forms]) -> bool {
    let n = norm(name);
    if n.is_empty() {
        return false;
    }
    if intersects(&keys(&n), forms) {
        return !others.iter().any(|o| intersects(&keys(&n), o));
    }
    if !approximates(&n, forms) {
        return false;
    }
    !others
        .iter()
        .any(|o| intersects(&keys(&n), o) || approximates(&n, o))
}
