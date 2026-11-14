const URL_SCHEMES: [&str; 4] = ["https://", "http://", "rtmp://", "rtsp://"];
const MIN_SECRET_LEN: usize = 4;

/// Replaces every configured secret and every URL with a placeholder, so a
/// stored record or an error shown to the operator never carries a credential.
pub fn mask(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for secret in secrets.iter().filter(|s| s.len() >= MIN_SECRET_LEN) {
        out = out.replace(secret.as_str(), "***");
    }
    mask_urls(&out)
}

pub fn mask_json(value: serde_json::Value, secrets: &[String]) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(s) => Value::String(mask(&s, secrets)),
        Value::Array(items) => Value::Array(items.into_iter().map(|v| mask_json(v, secrets)).collect()),
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, mask_json(v, secrets))).collect()),
        other => other,
    }
}

fn mask_urls(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((at, scheme)) = URL_SCHEMES.iter().filter_map(|s| rest.find(s).map(|i| (i, *s))).min_by_key(|(i, _)| *i) {
        out.push_str(&rest[..at]);
        out.push_str("<url>");
        let tail = &rest[at + scheme.len()..];
        let end = tail.find(|c: char| c.is_whitespace() || c == '"' || c == '\\' || c == '\'' || c == ')').unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

pub fn clip(text: &str, max: usize) -> String {
    let mut chars = text.chars();
    let head: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() { format!("{head}…") } else { head }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_and_urls_are_replaced() {
        let text = "call https://portal.example/get.php?u=1&p=2 failed with key sk-abcdef12";
        let masked = mask(text, &["sk-abcdef12".to_string()]);
        assert_eq!(masked, "call <url> failed with key ***");
    }

    #[test]
    fn a_short_secret_is_not_treated_as_one() {
        assert_eq!(mask("ok a b", &["a".to_string()]), "ok a b");
    }

    #[test]
    fn clipping_counts_characters() {
        assert_eq!(clip("ééééé", 3), "ééé…");
        assert_eq!(clip("éé", 3), "éé");
    }
}
