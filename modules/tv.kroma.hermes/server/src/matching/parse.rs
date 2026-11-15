pub fn parse_score(text: &str) -> Option<(i64, i64)> {
    let chars: Vec<char> = text.chars().collect();
    let digits = |from: usize| -> Option<(i64, usize)> {
        let mut end = from;
        while end < chars.len() && chars[end].is_ascii_digit() && end - from < 2 {
            end += 1;
        }
        (end > from).then(|| (chars[from..end].iter().collect::<String>().parse().unwrap_or(0), end))
    };
    for start in 0..chars.len() {
        if start > 0 && chars[start - 1].is_ascii_digit() {
            continue;
        }
        let Some((left, mut at)) = digits(start) else { continue };
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        if at >= chars.len() || !matches!(chars[at], '-' | '–' | ':') {
            continue;
        }
        at += 1;
        while at < chars.len() && chars[at].is_whitespace() {
            at += 1;
        }
        if let Some((right, _)) = digits(at) {
            return Some((left, right));
        }
    }
    None
}

pub fn parse_clock(text: &str) -> Option<i64> {
    let t = text.trim();
    if let Some((m, s)) = t.split_once(':') {
        let minutes = m.trim().parse::<i64>().ok().filter(|v| (0..1000).contains(v));
        let seconds = s.trim().parse::<i64>().ok().filter(|v| (0..60).contains(v));
        if let (Some(m), Some(s)) = (minutes, seconds) {
            return Some(m * 60 + s);
        }
    }
    if let Some((base, extra)) = t.split_once('+') {
        let clean = |x: &str| x.trim().trim_end_matches(['\'', '’']).trim().parse::<i64>().ok();
        if let (Some(b), Some(e)) = (clean(base), clean(extra)) {
            return Some((b + e) * 60);
        }
    }
    let bare = t.trim_end_matches("min").trim().trim_end_matches(['\'', '’']).trim();
    bare.parse::<i64>().ok().filter(|v| (0..1000).contains(v)).map(|m| m * 60)
}
