use kroma_module_sdk::host::HostCtx;

use crate::sport::FOOT;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    pub competitions: Vec<String>,
    pub tennis: bool,
    pub formula_one: bool,
    pub golf: bool,
    pub espn_base: String,
    pub poll_live_s: f64,
    pub poll_kickoff_s: f64,
    pub poll_quiet_s: f64,
    pub poll_idle_s: f64,
    pub match_poll_s: f64,
    pub detection_s: f64,
    pub spoiler_delay_s: f64,
    pub spoiler_margin_s: f64,
    pub reveal_margin_s: f64,
    pub prono: bool,
    pub goal_announcements: bool,
    pub clips: bool,
    pub clip_retention_days: i64,
    pub home_prefixes: Vec<String>,
    pub language: String,
}

fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

impl Settings {
    pub fn read(host: &dyn HostCtx) -> Self {
        let all: Vec<&str> = FOOT.iter().map(|(code, _, _)| *code).collect();
        let configured = list(&host.config_str("competitions", &all.join(",")));
        let competitions: Vec<String> = all
            .iter()
            .filter(|c| configured.iter().any(|x| x == *c))
            .map(ToString::to_string)
            .collect();
        let seconds = |key: &str, default: i64, low: i64| {
            host.config_i64(key, default).clamp(low, 86_400) as f64
        };
        Self {
            enabled: host.config_bool("enabled", true),
            competitions,
            tennis: host.config_bool("includeTennis", true),
            formula_one: host.config_bool("includeFormulaOne", true),
            golf: host.config_bool("includeGolf", true),
            espn_base: host
                .config_str("espnBaseUrl", "https://site.api.espn.com")
                .trim_end_matches('/')
                .to_string(),
            poll_live_s: seconds("pollLiveSeconds", 45, 10),
            poll_kickoff_s: seconds("pollKickoffSeconds", 90, 10),
            poll_quiet_s: seconds("pollQuietSeconds", 300, 30),
            poll_idle_s: seconds("pollIdleSeconds", 900, 60),
            match_poll_s: seconds("matchPollSeconds", 20, 5),
            detection_s: seconds("detectionSeconds", 30, 10),
            spoiler_delay_s: seconds("spoilerDelaySeconds", 60, 0),
            spoiler_margin_s: seconds("spoilerMarginSeconds", 15, 0),
            reveal_margin_s: seconds("revealMarginSeconds", 8, 0),
            prono: host.config_bool("pronoEnabled", true),
            goal_announcements: host.config_bool("goalAnnouncements", true),
            clips: host.config_bool("clipsEnabled", true),
            clip_retention_days: host.config_i64("clipRetentionDays", 45).clamp(1, 365),
            home_prefixes: list(&host.config_str("homeCountries", "fr,fra,france")),
            language: host.config_str("announceLanguage", "fr"),
        }
    }
}
