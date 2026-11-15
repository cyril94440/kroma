use regex::Regex;
use std::sync::LazyLock;

use crate::text::fold_keep_apostrophes;

const KNOWN_CHANNELS: [&str; 27] = [
    "canal+ foot",
    "canal+ sport 360",
    "canal+ sport",
    "canal+ premier league",
    "canal+ f1",
    "canal+",
    "bein sports 1",
    "bein sports 2",
    "bein sports 3",
    "bein sports max",
    "bein sports",
    "dazn",
    "ligue1+",
    "ligue 1+",
    "eurosport 1",
    "eurosport 2",
    "rmc sport 1",
    "rmc sport 2",
    "tf1",
    "france 2",
    "france 3",
    "m6",
    "w9",
    "l'equipe",
    "golf+",
    "prime video",
    "amazon prime",
];

static LIVE_NUMBERED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" (canal\+ live \d+) ").unwrap());
static LIVE_BARE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" canal\+ live ").unwrap());
static CHANNEL_NOISE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9+' ]+").unwrap());
static COUNTRY_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*([A-Za-z]{2,3})\s*[:|]").unwrap());

pub fn channel_in_text(text: &str) -> Option<String> {
    let kept = format!(
        " {} ",
        CHANNEL_NOISE.replace_all(&fold_keep_apostrophes(text), " ")
    );
    let mut found: Vec<(usize, i64, String)> = KNOWN_CHANNELS
        .iter()
        .filter_map(|c| {
            kept.find(&format!(" {c} "))
                .map(|at| (at, -(c.len() as i64), c.to_string()))
        })
        .collect();
    found.extend(LIVE_NUMBERED.captures_iter(&kept).filter_map(|m| {
        m.get(1).map(|g| {
            (
                g.start(),
                -(g.as_str().len() as i64),
                g.as_str().to_string(),
            )
        })
    }));
    if let Some(bare) = LIVE_BARE.find(&kept) {
        found.retain(|(at, _, name)| !(name == "canal+" && *at == bare.start()));
    }
    found.into_iter().min().map(|(_, _, name)| name)
}

pub fn is_foreign_channel(name: &str, home_prefixes: &[String]) -> bool {
    COUNTRY_PREFIX
        .captures(name)
        .is_some_and(|c| !home_prefixes.iter().any(|h| h.eq_ignore_ascii_case(&c[1])))
}
