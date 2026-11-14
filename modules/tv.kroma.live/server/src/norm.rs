use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

pub const TAG_TOKENS: &[&str] = &[
    "raw", "hd", "fhd", "sd", "uhd", "4k", "8k", "hevc", "h265", "h264", "vip", "be", "fr", "prime",
    "fps", "60fps", "50fps", "25fps", "1080i", "1080p", "720p", "hdr", "multi", "backup",
];

const FR_PREFIXES: &[&str] = &["fr", "fra", "france", "vip", "prime"];

static SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#{3,}|[•●★▼▲◆■]").unwrap());
static PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^a-z0-9]*([a-z]{2,6})\s*[:|]\s*").unwrap());
static TIMESHIFT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+1\b").unwrap());
static EVENT_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:live|next)\s*\|").unwrap());
static CLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b\d{1,2}[:h]\d{2}\b").unwrap());
static NAME_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{3,}$").unwrap());
static WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z0-9+]+").unwrap());
static ISOLATED_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|\s)(\d{1,2})(?:\s|$)").unwrap());
static COMPACT: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"ligue\s*1\s*\+", "ligue1+"),
        (r"\bligue\s+1\b", "ligue1"),
        (r"canal\s*plus\b", "canal+"),
        (r"canal\+([a-z])", "canal+ $1"),
        (r"\bsports\b", "sport"),
    ]
    .into_iter()
    .map(|(pattern, replacement)| (Regex::new(pattern).unwrap(), replacement))
    .collect()
});

pub fn fold(text: &str) -> String {
    let stripped: String = text.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    stripped.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn is_separator(name: &str) -> bool {
    SEPARATOR.is_match(name)
}

pub fn compact(normalised: &str) -> String {
    compact_except(normalised, None)
}

const PLAIN_LIGUE_ONE: usize = 1;

fn compact_except(normalised: &str, skipped: Option<usize>) -> String {
    COMPACT
        .iter()
        .enumerate()
        .filter(|(at, _)| Some(*at) != skipped)
        .fold(normalised.to_string(), |text, (_, (pattern, replacement))| {
            pattern.replace_all(&text, *replacement).into_owned()
        })
}

pub fn chan_number(normalised: &str) -> u32 {
    let cleaned = CLOCK.replace_all(&compact(normalised), " ").into_owned();
    ISOLATED_NUMBER
        .captures(&cleaned)
        .and_then(|found| found[1].parse().ok())
        .unwrap_or(999)
}

pub fn event_split(normalised: &str) -> Option<Vec<String>> {
    if !EVENT_HEAD.is_match(normalised) {
        return None;
    }
    let segments: Vec<String> = normalised.split('|').map(|s| s.trim().to_string()).collect();
    (segments.len() >= 4 && !segments.last().is_some_and(String::is_empty)).then_some(segments)
}

pub fn is_tag(token: &str) -> bool {
    TAG_TOKENS.contains(&token)
}

fn strip_prefixes(mut text: String) -> (String, String) {
    let mut region = "fr".to_string();
    for _ in 0..2 {
        let Some(found) = PREFIX.captures(&text) else { break };
        let country = found[1].to_string();
        if !FR_PREFIXES.contains(&country.as_str()) {
            region = country;
        }
        let end = found.get(0).map_or(0, |m| m.end());
        text = text[end..].to_string();
    }
    (text, region)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelKey {
    pub region: String,
    pub words: BTreeSet<String>,
}

pub fn channel_key(normalised: &str) -> ChannelKey {
    let base = event_split(normalised).and_then(|s| s.last().cloned()).unwrap_or_else(|| normalised.to_string());
    let (text, region) = strip_prefixes(base);
    let text = compact_except(&text, Some(PLAIN_LIGUE_ONE));
    let text = TIMESHIFT.replace_all(&text, " +1 ").into_owned();
    let words = WORDS
        .find_iter(&text)
        .map(|m| m.as_str().to_string())
        .filter(|word| !is_tag(word))
        .collect();
    ChannelKey { region, words }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Family {
    pub region: String,
    pub head: String,
}

pub type Slot = (Family, Option<u32>);

pub fn family_slot(normalised: &str) -> Slot {
    let base = event_split(normalised).and_then(|s| s.last().cloned()).unwrap_or_else(|| normalised.to_string());
    let (text, region) = strip_prefixes(base);
    let before_dash = text.split(" - ").next().unwrap_or("").to_string();
    let text = compact(&CLOCK.replace_all(&before_dash, " "));
    let mut number = None;
    let mut head = Vec::new();
    for token in WORDS.find_iter(&text).map(|m| m.as_str()) {
        if is_tag(token) {
            continue;
        }
        let digits = token.bytes().all(|b| b.is_ascii_digit());
        if number.is_none() && digits && token.len() <= 2 {
            number = token.parse().ok();
        } else if !digits || is_name_number(token) {
            head.push(token.to_string());
        }
    }
    (Family { region, head: head.join(" ") }, number)
}

fn is_name_number(token: &str) -> bool {
    NAME_NUMBER.is_match(token) && !matches!(token, "720" | "1080" | "2160" | "4320") && !is_year(token)
}

fn is_year(token: &str) -> bool {
    token.len() == 4 && (token.starts_with("19") || token.starts_with("20"))
}
