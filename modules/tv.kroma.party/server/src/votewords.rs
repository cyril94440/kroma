const THUMBS_UP: &str = "\u{1F44D}";
const THUMBS_DOWN: &str = "\u{1F44E}";

const YES: &[&str] = &[
    "oui", "ouais", "ok", "okay", "d'accord", "daccord", "yes", "vas-y", "vasy", "go", "+1", "yep",
    "carrément", "carrement",
];
const NO: &[&str] = &["non", "nan", "no", "nope", "not", "-1"];
const NO_PHRASES: &[&str] = &["surtout pas", "pas d'accord", "pas daccord", "pas question"];
const NEGATED_YES: &[&str] = &["ok", "okay", "go", "oui", "ouais", "yes", "vas-y"];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '\'' | '+' | '-')
}

fn words(text: &str) -> Vec<(usize, usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (is_word_char(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i, &text[s..i]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len(), &text[s..]));
    }
    out
}

fn negated_yes(text: &str, tokens: &[(usize, usize, &str)]) -> bool {
    tokens.windows(2).any(|pair| {
        let (_, end, first) = pair[0];
        let (start, _, second) = pair[1];
        let gap = &text[end..start];
        let spaced = !gap.is_empty() && gap.chars().all(char::is_whitespace);
        let agrees = NEGATED_YES.contains(&second)
            || second.starts_with("d'acc")
            || second.starts_with("dacc");
        spaced && (first == "pas" || first == "jamais") && agrees
    })
}

/// Reads a typed vote: `Some(true)` for, `Some(false)` against, `None` when the
/// message is not a vote. Any doubt is not a "for".
pub fn parse_vote(text: &str) -> Option<bool> {
    let t = text.to_lowercase().trim().replace('\u{2019}', "'");
    if t.contains(THUMBS_DOWN) {
        return Some(false);
    }
    let tokens = words(&t);
    if NO_PHRASES.iter().any(|p| t.contains(p)) || negated_yes(&t, &tokens) {
        return Some(false);
    }
    let ws: Vec<&str> = tokens.iter().map(|(_, _, w)| *w).collect();
    let has_no = ws.iter().any(|w| NO.contains(w));
    if ws.first().is_some_and(|w| NO.contains(w)) {
        return Some(false);
    }
    if has_no && ws.len() <= 3 {
        return Some(false);
    }
    if has_no || ws.contains(&"pas") || ws.contains(&"jamais") {
        return None;
    }
    if t.contains(THUMBS_UP) {
        return Some(true);
    }
    if ws.is_empty() {
        return None;
    }
    if ws[0] == "oui" {
        return Some(true);
    }
    if ws.len() > 3 {
        return None;
    }
    if ws.iter().any(|w| YES.contains(w)) {
        return Some(true);
    }
    None
}

#[cfg(test)]
mod tests;
