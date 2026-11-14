pub const TEXT_MAX: usize = 500;
pub const AGENT_TEXT_MAX: usize = 1000;
pub const HISTORY_MAX: usize = 100;
pub const RETENTION_MS: i64 = 24 * 3600 * 1000;
pub const RATE_COUNT: usize = 8;
pub const RATE_WINDOW_MS: i64 = 8000;
pub const REACTION_WINDOW: usize = 200;
pub const FLOATING_RATE_COUNT: usize = 8;
pub const FLOATING_RATE_WINDOW_MS: i64 = 3000;
pub const FLOATING_EMOJI_MAX: usize = 8;

pub const REACTION_EMOJIS: &[&str] = &[
    "\u{26BD}",
    "\u{1F525}",
    "\u{1F602}",
    "\u{1F62E}",
    "\u{2764}\u{FE0F}",
    "\u{1F44F}",
];

const KEYWORDS: &[&str] = &[
    "hermes", "hermès", "mets", "met ", "mettre", "metter", "zappe", "zapp", "change", "changer",
    "passe", "affiche", "affich", "chaine", "chaîne", "chaines", "chaînes", "match", "foot",
    "regarder", "regarde", "voir", "diffuse", "programme", "dessin", "film", "série", "serie",
    "bug", "saccade", "rame", "lag", "figé", "fige", "freeze", "plante", "planté", "cassé", "casse",
    "coupe", "coupé", "coupure", "gèle", "gele", "gelé", "son ", "image", "écran", "ecran",
    "combien", "remets", "remet", "scan", "teste", "tester", "secours", "fournisseur", "source",
    "flux", "noir", "cherche", "trouve", "compare", "vérifie", "verifie", "classement", "score",
    "résultat", "resultat", "quel ", "quelle", "qui joue", "qui gagne",
];

pub fn clip(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

pub fn mentions_agent(text: &str, agent_name: &str) -> bool {
    let t = text.to_lowercase();
    let name = agent_name.trim().to_lowercase();
    ["hermes", "hermès"].iter().any(|n| t.contains(n)) || (!name.is_empty() && t.contains(&name))
}

pub fn is_question(text: &str) -> bool {
    text.trim().ends_with('?')
}

pub fn keyword_hit(text: &str) -> bool {
    let t = text.to_lowercase();
    KEYWORDS.iter().any(|k| t.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_is_named_by_its_name_or_the_configured_one() {
        assert!(mentions_agent("Hermès, mets TF1", "Hermès"));
        assert!(mentions_agent("salut Zeus", "Zeus"));
        assert!(!mentions_agent("on mange quoi", "Hermès"));
    }

    #[test]
    fn lexical_signals_follow_the_source_gate() {
        assert!(is_question("c'est quoi ce film ?"));
        assert!(!is_question("bonjour"));
        assert!(keyword_hit("la chaîne bug"));
        assert!(!keyword_hit("bonsoir tout le monde"));
    }

    #[test]
    fn a_message_is_clipped_by_characters() {
        assert_eq!(clip("  \u{e9}\u{e9}\u{e9}\u{e9}  ", 2), "\u{e9}\u{e9}");
    }
}
