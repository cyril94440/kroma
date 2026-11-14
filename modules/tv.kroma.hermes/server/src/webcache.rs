use std::collections::HashMap;
use std::sync::Mutex;

pub const TTL_MS: i64 = 600_000;
const CACHE_MAX: usize = 64;

fn fold_char(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'â' | 'ä' | 'á' => "a",
        'ç' => "c",
        'é' | 'è' | 'ê' | 'ë' => "e",
        'î' | 'ï' => "i",
        'ô' | 'ö' => "o",
        'ù' | 'û' | 'ü' => "u",
        'ÿ' => "y",
        'œ' => "oe",
        'æ' => "ae",
        _ => return None,
    })
}

fn fold(text: &str) -> String {
    text.to_lowercase().chars().fold(String::new(), |mut out, c| {
        match fold_char(c) {
            Some(s) => out.push_str(s),
            None => out.push(c),
        }
        out
    })
}

/// Lowercase, accent-free text reduced to single-spaced alphanumeric words:
/// the key of every cache and the form every word list is matched against.
pub fn normalise(text: &str) -> String {
    let folded: String = fold(text).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' }).collect();
    folded.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct TtlCache<T> {
    ttl: i64,
    max: usize,
    entries: Mutex<HashMap<String, (i64, T)>>,
}

impl<T: Clone> TtlCache<T> {
    pub fn new(ttl: i64) -> Self {
        Self { ttl, max: CACHE_MAX, entries: Mutex::new(HashMap::new()) }
    }

    pub fn with_capacity(ttl: i64, max: usize) -> Self {
        Self { ttl, max, entries: Mutex::new(HashMap::new()) }
    }

    pub fn get(&self, key: &str, now: i64) -> Option<T> {
        let key = normalise(key);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        match entries.get(&key) {
            Some((at, value)) if now - at < self.ttl => Some(value.clone()),
            Some(_) => {
                entries.remove(&key);
                None
            }
            None => None,
        }
    }

    pub fn put(&self, key: &str, value: T, now: i64) {
        let key = normalise(key);
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if !entries.contains_key(&key) && entries.len() >= self.max {
            if let Some(oldest) = entries.iter().min_by_key(|(_, (at, _))| *at).map(|(k, _)| k.clone()) {
                entries.remove(&oldest);
            }
        }
        entries.insert(key, (now, value));
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

const DATED_WORDS: [&str; 11] = ["score", "resultat", "gagne", "battu", "mene", "horaire", "heure", "commence", "marque", "but", "buts"];
const WHEN_VERBS: [&str; 9] = ["joue", "jouent", "commence", "commencent", "passe", "passent", "reprend", "commencen", "commencet"];

fn at<'a>(words: &[&'a str], i: usize) -> &'a str {
    words.get(i).copied().unwrap_or("")
}

/// A live score or a schedule: an answer to it without a dated mark may be
/// about another match, so it is never served from the cache nor trusted.
pub fn question_dated(question: &str) -> bool {
    let n = normalise(question);
    let w: Vec<&str> = n.split(' ').collect();
    (0..w.len()).any(|i| {
        let t = w[i];
        DATED_WORDS.contains(&t)
            || (t == "mi" && at(&w, i + 1) == "temps")
            || (t == "coup" && at(&w, i + 1) == "d" && at(&w, i + 2) == "envoi")
            || (t == "quand" && (WHEN_VERBS.contains(&at(&w, i + 1)) || (at(&w, i + 1) == "a" && at(&w, i + 2) == "lieu")))
            || (WHEN_VERBS.contains(&t) && at(&w, i + 1) == "quand")
            || (t == "c" && at(&w, i + 1) == "est" && at(&w, i + 2) == "quand")
            || (t == "quand" && at(&w, i + 1) == "est" && at(&w, i + 2) == "ce" && (3..6).any(|k| WHEN_VERBS.contains(&at(&w, i + k))))
            || (t == "match" && is_today_phrase(&w, i + 1))
    })
}

fn is_today_phrase(w: &[&str], mut i: usize) -> bool {
    if at(w, i) == "de" {
        i += 1;
    }
    (at(w, i) == "ce" && at(w, i + 1) == "soir") || (at(w, i) == "aujourd" && at(w, i + 1) == "hui") || at(w, i) == "aujourdhui" || at(w, i) == "demain"
}

const MONTHS_AND_DAYS: [&str; 19] = [
    "janvier", "fevrier", "mars", "avril", "mai", "juin", "juillet", "aout", "septembre", "octobre", "novembre", "decembre", "lundi", "mardi", "mercredi", "jeudi", "vendredi",
    "samedi", "dimanche",
];

fn digits_before(chars: &[char], end: usize) -> usize {
    let run = chars[..end].iter().rev().take_while(|c| c.is_ascii_digit()).count();
    let boundary = end < run + 1 || !chars[end - run - 1].is_alphanumeric();
    if boundary { run } else { 0 }
}

fn clock_at(chars: &[char], i: usize) -> bool {
    let lead = if i > 0 && chars[i - 1] == ' ' { i - 1 } else { i };
    if !(1..=2).contains(&digits_before(chars, lead)) || chars.get(i + 1).is_some_and(|c| c.is_alphabetic()) {
        return false;
    }
    let start = if chars.get(i + 1) == Some(&' ') { i + 2 } else { i + 1 };
    let run = chars.iter().skip(start).take_while(|c| c.is_ascii_digit()).count();
    run == 0 || (run <= 2 && !chars.get(start + run).is_some_and(|c| c.is_alphanumeric()))
}

fn has_clock_mark(chars: &[char]) -> bool {
    (0..chars.len()).any(|i| match chars[i] {
        'h' => clock_at(chars, i),
        '/' => (1..=2).contains(&digits_before(chars, i)) && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()),
        _ => false,
    })
}

/// Whether an answer carries a date, an hour, a weekday, a month or a match
/// minute. A bare year counts only when it is the current one: a score "in
/// 2024" is a stale one.
pub fn has_date_mark(text: &str, year: i64) -> bool {
    let folded = fold(text);
    let chars: Vec<char> = folded.chars().collect();
    if has_clock_mark(&chars) {
        return true;
    }
    let words: Vec<&str> = folded.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
    let year = year.to_string();
    words.iter().enumerate().any(|(i, t)| {
        MONTHS_AND_DAYS.contains(t)
            || ["hier", "demain", "minute", "aujourdhui"].contains(t)
            || (*t == "ce" && at(&words, i + 1) == "soir")
            || (*t == "aujourd" && at(&words, i + 1) == "hui")
            || (*t == "mi" && at(&words, i + 1) == "temps")
            || ((*t == "heure" || *t == "heures") && i > 0 && words[i - 1].len() <= 2 && words[i - 1].chars().all(|c| c.is_ascii_digit()))
            || *t == year
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub rank: f64,
    pub title: String,
    pub matched: usize,
    pub channel_id: String,
    pub channel_name: String,
    pub start: i64,
    pub end: i64,
    pub domestic: bool,
}

/// Orders in place: foreign channels last, then the title equal to the
/// request, then the best channel; the sort is stable so the guide order
/// settles the rest.
pub fn exact_title_first(entries: &mut [Entry], requested: &str) {
    let wanted = normalise(requested);
    entries.sort_by(|a, b| {
        let key = |e: &Entry| (!e.domestic, normalise(&e.title) != wanted);
        key(a).cmp(&key(b)).then(b.rank.total_cmp(&a.rank))
    });
}

/// The programmes of different titles that rank with the first one, between
/// which nobody should guess; empty when a title equals the request, or when
/// one title is merely on several channels.
pub fn ambiguous<'a>(entries: &'a [Entry], requested: &str, class: impl Fn(&Entry) -> f64) -> Vec<&'a Entry> {
    let Some(first) = entries.first() else { return Vec::new() };
    if entries.len() < 2 || normalise(&first.title) == normalise(requested) {
        return Vec::new();
    }
    let class_first = class(first);
    let mut titles: Vec<String> = Vec::new();
    let equal: Vec<&Entry> = entries
        .iter()
        .filter(|e| class(e) == class_first && e.matched == first.matched)
        .filter(|e| {
            let t = normalise(&e.title);
            !titles.contains(&t) && {
                titles.push(t);
                true
            }
        })
        .collect();
    if equal.len() > 1 { equal } else { Vec::new() }
}

/// A channel name prefixed by a country code other than France ("UK: ...").
pub fn is_foreign(channel_name: &str) -> bool {
    let name = channel_name.trim();
    let Some(end) = name.find([':', '|']).filter(|i| *i <= 4) else { return false };
    let code = name[..end].trim().to_lowercase();
    (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_alphabetic()) && !["fr", "fra"].contains(&code.as_str())
}

#[cfg(test)]
mod tests;
