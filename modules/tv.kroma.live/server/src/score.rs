use std::sync::LazyLock;

use regex::Regex;

use crate::model::Channel;
use crate::rules::Rules;

static COUNTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^a-z0-9]*([a-z]{2,5})\b").unwrap());
static EIGHT_K: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b8k\b").unwrap());
static ALIASES: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        (Regex::new(r"\bl1\b").unwrap(), "ligue1"),
        (Regex::new(r"(\w)\s*plus\b").unwrap(), "${1}+"),
    ]
});

pub fn asks_4k(query: &str) -> bool {
    query.contains("4k") || query.contains("uhd") || EIGHT_K.is_match(query)
}

pub fn expand_query(query: &str) -> String {
    let expanded = ALIASES
        .iter()
        .fold(query.to_string(), |text, (pattern, replacement)| pattern.replace_all(&text, *replacement).into_owned());
    expanded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn country_score(normalised: &str, rules: &Rules) -> f64 {
    let code = COUNTRY.captures(normalised).map(|found| found[1].to_string()).unwrap_or_default();
    if rules.home_country.contains(&code) {
        rules.home_bonus
    } else if rules.foreign_codes.contains(&code) {
        -rules.foreign_penalty
    } else {
        0.0
    }
}

pub fn preference(normalised: &str, rules: &Rules) -> f64 {
    let mut score = country_score(normalised, rules);
    if normalised.contains("raw") {
        score += rules.raw;
    }
    if normalised.contains("vip") {
        score += rules.vip;
    }
    if normalised.contains("fhd") {
        score += rules.fhd;
    } else if normalised.contains("hd") {
        score += rules.hd;
    }
    score
}

pub fn query_score(channel: &Channel, query: &str, rules: &Rules) -> f64 {
    let mut score = preference(&channel.norm, rules);
    if channel.norm == query {
        score += rules.exact;
    }
    score - channel.norm.chars().count() as f64 * 0.002
}
