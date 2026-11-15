use std::sync::LazyLock;

use regex::Regex;

pub const IMAGE_MARGIN_MS: i64 = 3000;
pub const POSSESSION_MAX: usize = 10;
pub const DEFAULT_COLOR: &str = "#6b6256";

pub fn period_start_s(period: i64) -> Option<i64> {
    match period {
        1 => Some(0),
        2 => Some(2700),
        3 => Some(5400),
        4 => Some(6300),
        5 => Some(7200),
        _ => None,
    }
}

fn regulation_end_s(minute: i64) -> Option<i64> {
    match minute {
        45 => Some(2700),
        90 => Some(5400),
        105 => Some(6300),
        120 => Some(7200),
        _ => None,
    }
}

static STOPPAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d+)\s*['’]?\s*\+\s*(\d+)").unwrap());

pub fn stoppage_second(display: &str) -> Option<(i64, i64)> {
    let m = STOPPAGE.captures(display)?;
    let end = regulation_end_s(m[1].parse().ok()?)?;
    let period = (1..=5).find(|p| period_start_s(*p) == Some(end))? - 1;
    Some((end + m[2].parse::<i64>().ok()? * 60, period))
}

pub fn contract_type(text: &str) -> Option<&'static str> {
    let text = text.trim();
    let goal = text.eq_ignore_ascii_case("goal")
        || (text.len() > 4
            && text[..4].eq_ignore_ascii_case("goal")
            && text[4..].trim_start().starts_with('-'))
        || text == "Penalty - Scored"
        || text == "Own Goal";
    if goal {
        return Some("but");
    }
    match text {
        "Shot On Target" => Some("tir_cadre"),
        "Shot Off Target" => Some("tir"),
        "Shot Blocked" => Some("tir_contre"),
        "Corner Awarded" => Some("corner"),
        "Yellow Card" => Some("jaune"),
        "Red Card" | "Second Yellow" => Some("rouge"),
        "Substitution" => Some("remplacement"),
        _ => None,
    }
}

pub fn color(candidates: &[&str]) -> String {
    for c in candidates {
        let hex = c.trim_start_matches('#');
        if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return format!("#{}", hex.to_lowercase());
        }
    }
    DEFAULT_COLOR.into()
}

pub fn period_k(readings: &[(f64, f64)]) -> Option<f64> {
    let mut values: Vec<f64> = readings
        .iter()
        .map(|(ts, chrono)| ts * 1000.0 - chrono * 1000.0)
        .collect();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let n = values.len();
    Some(if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    })
}
