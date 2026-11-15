use std::collections::HashSet;
use std::sync::LazyLock;

const ALIASES: &[(&str, &str)] = &[
    ("psg", "psg"), ("paris sg", "psg"), ("paris saint germain", "psg"), ("paris st germain", "psg"),
    ("pfc", "paris fc"), ("paris fc", "paris fc"),
    ("rcl", "lens"), ("rc lens", "lens"), ("racing club de lens", "lens"),
    ("om", "marseille"), ("olympique de marseille", "marseille"),
    ("ol", "lyon"), ("olympique lyonnais", "lyon"), ("lyonnais", "lyon"),
    ("losc", "lille"), ("losc lille", "lille"),
    ("asm", "monaco"), ("as monaco", "monaco"),
    ("ogcn", "nice"), ("ogc nice", "nice"),
    ("srfc", "rennes"), ("stade rennais", "rennes"), ("rennais", "rennes"),
    ("fcn", "nantes"), ("fc nantes", "nantes"),
    ("rcsa", "strasbourg"), ("rc strasbourg", "strasbourg"), ("strasbourg alsace", "strasbourg"),
    ("tfc", "toulouse"),
    ("hac", "le havre"), ("lhac", "le havre"), ("havre", "le havre"), ("le havre ac", "le havre"),
    ("aja", "auxerre"), ("aj auxerre", "auxerre"),
    ("fcl", "lorient"),
    ("sco", "angers"), ("angers sco", "angers"),
    ("fcm", "metz"), ("fc metz", "metz"),
    ("sb29", "brest"), ("brestois", "brest"), ("stade brestois", "brest"),
    ("asse", "saint etienne"), ("as saint etienne", "saint etienne"), ("st etienne", "saint etienne"),
    ("sdr", "reims"), ("stade de reims", "reims"),
    ("inter", "inter"), ("inter milan", "inter"), ("internazionale", "inter"),
    ("juve", "juventus"),
    ("barca", "barcelone"), ("barcelona", "barcelone"), ("fc barcelone", "barcelone"),
    ("real", "real madrid"), ("rma", "real madrid"),
    ("bayern munich", "bayern"), ("bayern munchen", "bayern"), ("fc bayern", "bayern"), ("bayern", "bayern"),
    ("bvb", "dortmund"), ("borussia dortmund", "dortmund"),
    ("man city", "manchester city"), ("man utd", "manchester united"), ("man united", "manchester united"),
    ("atletico", "atletico madrid"), ("atletico de madrid", "atletico madrid"), ("atleti", "atletico madrid"),
];

const EXTRA_KNOWN: &[&str] = &[
    "lille", "lens", "nice", "rennes", "nantes", "lorient", "metz", "brest", "angers", "auxerre", "toulouse", "monaco", "lyon", "marseille", "strasbourg", "reims", "le havre", "paris fc",
    "saint etienne", "montpellier", "nimes", "bordeaux",
];

const GENERIC: &[&str] = &["fc", "ac", "as", "sc", "rc", "ogc", "us", "afc", "cf", "club", "football", "stade", "olympique", "racing", "de", "du", "la", "le", "les", "l", "d", "sporting"];

pub fn alias(folded: &str) -> Option<&'static str> {
    ALIASES.iter().find(|(from, _)| *from == folded).map(|(_, to)| *to)
}

pub fn is_generic(word: &str) -> bool {
    GENERIC.contains(&word)
}

pub fn is_ambiguous(name: &str) -> bool {
    name == "paris"
}

static KNOWN: LazyLock<HashSet<&'static str>> = LazyLock::new(|| ALIASES.iter().map(|(_, to)| *to).chain(EXTRA_KNOWN.iter().copied()).collect());

pub fn is_known(name: &str) -> bool {
    KNOWN.contains(name)
}
