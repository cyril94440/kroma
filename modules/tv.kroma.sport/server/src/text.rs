use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9]+").unwrap());
static NOT_WORD_PLUS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-z0-9+]+").unwrap());

fn transliterate(c: char) -> Option<&'static str> {
    Some(match c {
        'ø' | 'Ø' => "o",
        'æ' | 'Æ' => "ae",
        'œ' | 'Œ' => "oe",
        'ß' => "ss",
        'ł' | 'Ł' => "l",
        'đ' | 'Đ' | 'ð' => "d",
        'þ' => "th",
        'ı' => "i",
        _ => return None,
    })
}

fn spread(text: &str) -> String {
    text.chars().fold(String::new(), |mut out, c| {
        match transliterate(c) {
            Some(plain) => out.push_str(plain),
            None => out.push(c),
        }
        out
    })
}

fn strip_marks(text: &str) -> String {
    text.nfkd()
        .filter(|c| !is_combining_mark(*c) && c.is_ascii())
        .collect::<String>()
        .to_lowercase()
}

pub fn norm(text: &str) -> String {
    NOT_WORD
        .replace_all(&strip_marks(&spread(text)), " ")
        .trim()
        .to_string()
}

pub fn norm_lossy(text: &str) -> String {
    NOT_WORD
        .replace_all(&strip_marks(text), " ")
        .trim()
        .to_string()
}

pub fn fold(text: &str) -> String {
    NOT_WORD_PLUS
        .replace_all(&strip_marks(&spread(text)), " ")
        .trim()
        .to_string()
}

pub fn fold_keep_apostrophes(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

pub fn contains_words(haystack: &str, needle: &str) -> bool {
    !needle.is_empty() && format!(" {haystack} ").contains(&format!(" {needle} "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_transliterates_before_stripping_accents() {
        assert_eq!(norm("Bodø/Glimt"), "bodo glimt");
    }

    #[test]
    fn norm_lossy_drops_letters_without_a_decomposition() {
        assert_eq!(norm_lossy("Bodø/Glimt"), "bod glimt");
    }

    #[test]
    fn fold_keeps_the_plus_of_a_channel_name() {
        assert_eq!(fold("Canal+ Foot"), "canal+ foot");
    }

    #[test]
    fn contains_words_matches_whole_words_only() {
        assert!(contains_words("le havre psg", "psg"));
        assert!(!contains_words("psgx lens", "psg"));
    }
}
