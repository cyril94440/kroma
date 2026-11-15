use std::collections::BTreeSet;

use crate::standings::norm_club;

fn key(text: &str) -> String {
    match norm_club(text).as_str() {
        "paris sg" => "paris saint germain".into(),
        "marseille om" => "marseille".into(),
        other => other.to_string(),
    }
}

pub fn side_matches(side: &str, names: &[String]) -> bool {
    let wanted = key(side);
    let words: BTreeSet<&str> = wanted.split(' ').collect();
    names.iter().filter(|n| !n.is_empty()).any(|name| {
        let k = key(name);
        let theirs: BTreeSet<&str> = k.split(' ').collect();
        !k.is_empty() && (k == wanted || (theirs.len() >= 2 && theirs.is_subset(&words)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paris_fc_is_not_the_psg() {
        let psg: Vec<String> = ["Paris Saint-Germain", "Paris SG", "PSG"]
            .map(String::from)
            .to_vec();
        let paris_fc: Vec<String> = ["Paris FC", "Paris FC", "PFC"].map(String::from).to_vec();

        assert!(side_matches("PSG", &psg));
        assert!(side_matches("Paris SG", &psg));
        assert!(!side_matches("PSG", &paris_fc));
    }
}
