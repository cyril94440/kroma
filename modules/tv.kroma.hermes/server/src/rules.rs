use anyhow::{bail, Result};

use crate::store::Store;

pub const DEFAULT_RULES: &str = include_str!("../../resources/house_rules.md");
const LESSONS_SEED: &str = include_str!("../../resources/lessons_seed.txt");
const RULES_KEY: &str = "house_rules";
const SEEDED_KEY: &str = "lessons_seeded";
pub const LESSONS_HEADER: &str = "## Leçons apprises (par Hermès)";
pub const RULES_MAX_CHARS: usize = 20_000;

/// The house rules: an editable text (the shipped default until an
/// administrator saves one) followed by the lessons the assistant learned.
#[derive(Clone)]
pub struct Rules {
    store: Store,
}

impl Rules {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn text(&self) -> String {
        self.store.kv_get(RULES_KEY).ok().flatten().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| DEFAULT_RULES.to_string())
    }

    pub fn is_customised(&self) -> bool {
        self.store.kv_get(RULES_KEY).ok().flatten().is_some()
    }

    pub fn save(&self, text: &str) -> Result<()> {
        if text.chars().count() > RULES_MAX_CHARS {
            bail!("the house rules are limited to {RULES_MAX_CHARS} characters");
        }
        self.store.kv_set(RULES_KEY, text)
    }

    pub fn reset(&self) -> Result<()> {
        self.store.kv_delete(RULES_KEY)
    }

    pub fn seed_lessons(&self, now: i64) -> Result<()> {
        if self.store.kv_get(SEEDED_KEY)?.is_some() {
            return Ok(());
        }
        for line in LESSONS_SEED.lines().filter(|l| !l.trim().is_empty()) {
            self.store.add_lesson(line.trim(), now, crate::tools::LESSONS_MAX)?;
        }
        self.store.kv_set(SEEDED_KEY, "1")
    }

    pub fn render(&self) -> String {
        let lessons = self.store.lessons().unwrap_or_default();
        let mut out = self.text().trim_end().to_string();
        if !lessons.is_empty() {
            out.push_str(&format!("\n\n{LESSONS_HEADER}\n"));
            out.push_str(&lessons.iter().map(|l| format!("- {}", l.text)).collect::<Vec<_>>().join("\n"));
        }
        out
    }
}
