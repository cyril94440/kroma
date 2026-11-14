use std::fmt;

use kroma_module_sdk::host::HostCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Gemini,
    OpenAi,
}

#[derive(Clone, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub provider: ProviderKind,
    pub gemini_key: String,
    pub gemini_model: String,
    pub gemini_tier: String,
    pub gemini_base: String,
    pub openai_base: String,
    pub openai_key: String,
    pub openai_model: String,
    pub temperature: Option<f64>,
    pub language: String,
    pub max_steps: usize,
    pub turn_budget_s: i64,
    pub model_timeout_s: u32,
    pub engage_window_s: i64,
    pub hourly_calls: u32,
    pub user_turns_per_minute: u32,
    pub vision: bool,
    pub audit_days: i64,
}

impl Settings {
    pub fn read(host: &dyn HostCtx) -> Self {
        Self {
            enabled: host.config_bool("enabled", true),
            provider: if host.config_str("provider", "gemini") == "openai" { ProviderKind::OpenAi } else { ProviderKind::Gemini },
            gemini_key: host.config_str("geminiApiKey", "").trim().to_string(),
            gemini_model: host.config_str("geminiModel", "gemini-3.8-flash").trim().to_string(),
            gemini_tier: host.config_str("geminiServiceTier", "priority").trim().to_lowercase(),
            gemini_base: base(&host.config_str("geminiBaseUrl", ""), "https://generativelanguage.googleapis.com"),
            openai_base: base(&host.config_str("openaiBaseUrl", ""), "https://api.openai.com/v1"),
            openai_key: host.config_str("openaiApiKey", "").trim().to_string(),
            openai_model: host.config_str("openaiModel", "").trim().to_string(),
            temperature: host.config_str("temperature", "0").trim().parse().ok(),
            language: if host.config_str("language", "fr") == "en" { "en".into() } else { "fr".into() },
            max_steps: host.config_i64("maxSteps", 10).clamp(1, 20) as usize,
            turn_budget_s: host.config_i64("turnBudgetSeconds", 150).clamp(20, 600),
            model_timeout_s: host.config_i64("modelTimeoutSeconds", 40).clamp(1, 120) as u32,
            engage_window_s: host.config_i64("engageWindowSeconds", 90).clamp(0, 900),
            hourly_calls: host.config_i64("hourlyCallLimit", 200).clamp(1, 100_000) as u32,
            user_turns_per_minute: host.config_i64("userTurnsPerMinute", 4).clamp(1, 60) as u32,
            vision: host.config_bool("visionEnabled", true),
            audit_days: host.config_i64("auditDays", 30).clamp(1, 365),
        }
    }

    pub fn active_key(&self) -> &str {
        match self.provider {
            ProviderKind::Gemini => &self.gemini_key,
            ProviderKind::OpenAi => &self.openai_key,
        }
    }

    pub fn active_model(&self) -> &str {
        match self.provider {
            ProviderKind::Gemini => &self.gemini_model,
            ProviderKind::OpenAi => &self.openai_model,
        }
    }

    pub fn secrets(&self) -> Vec<String> {
        [&self.gemini_key, &self.openai_key].into_iter().filter(|k| !k.is_empty()).cloned().collect()
    }

    pub fn configured(&self) -> bool {
        !self.active_model().is_empty() && (!self.active_key().is_empty() || self.provider == ProviderKind::OpenAi)
    }
}

fn base(value: &str, fallback: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() { fallback } else { value }.to_string()
}

impl fmt::Debug for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Settings")
            .field("enabled", &self.enabled)
            .field("provider", &self.provider)
            .field("model", &self.active_model())
            .field("key", &if self.active_key().is_empty() { "unset" } else { "set" })
            .finish_non_exhaustive()
    }
}
