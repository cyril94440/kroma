use kroma_module_sdk::host::HostCtx;

use crate::cmd::{AudioMode, Options, DEFAULT_USER_AGENT};
use crate::model::ProviderId;
use crate::portal::Portal;

pub const BACKUP_USER_AGENT: &str = "IPTVSmartersPro";

#[derive(Debug, Clone)]
pub struct Settings {
    pub primary_url: String,
    pub backup_url: String,
    pub user_agent: String,
    pub backup_user_agent: String,
    pub epg_url: String,
    pub standby_seconds: u64,
    pub dead_after: f64,
    pub trouble_dead: f64,
    pub unstable_after: f64,
    pub unstable_window: f64,
    pub trial_after: f64,
    pub trial_gap: f64,
    pub trial_ratio: f64,
    pub min_kbps: f64,
    pub backup_max_connections: i64,
    pub backup_scan_with_live: bool,
    pub audio_auto_copy: String,
    pub default_channel: String,
    pub backup_enabled_failover: bool,
}

fn bounded(value: i64, default: f64, low: f64, high: f64) -> f64 {
    let value = value as f64;
    if value < low || value > high { default } else { value }
}

impl Settings {
    pub fn read(host: &dyn HostCtx) -> Settings {
        Settings {
            primary_url: host.config_str("primaryUrl", "").trim().to_string(),
            backup_url: host.config_str("backupUrl", "").trim().to_string(),
            user_agent: host.config_str("userAgent", DEFAULT_USER_AGENT).trim().to_string(),
            backup_user_agent: host.config_str("backupUserAgent", BACKUP_USER_AGENT).trim().to_string(),
            epg_url: host.config_str("epgUrl", "").trim().to_string(),
            standby_seconds: host.config_i64("standbySeconds", 90).clamp(5, 86_400) as u64,
            dead_after: bounded(host.config_i64("deadAfterSeconds", 45), 45.0, 15.0, 600.0),
            trouble_dead: bounded(host.config_i64("troubleDeadSeconds", 20), 20.0, 5.0, 120.0),
            unstable_after: bounded(host.config_i64("unstableAfterSeconds", 120), 120.0, 30.0, 1800.0),
            unstable_window: bounded(host.config_i64("unstableWindowSeconds", 300), 300.0, 60.0, 3600.0),
            trial_after: bounded(host.config_i64("trialAfterSeconds", 20), 20.0, 5.0, 600.0),
            trial_gap: host.config_str("trialGapSecondsPerMinute", "2").parse().ok().filter(|v| (0.5..=30.0).contains(v)).unwrap_or(2.0),
            trial_ratio: host.config_str("trialRatio", "3").parse().ok().filter(|v| (1.5..=20.0).contains(v)).unwrap_or(3.0),
            min_kbps: bounded(host.config_i64("minKbps", 100), 100.0, 1.0, 5000.0),
            backup_max_connections: host.config_i64("backupMaxConnections", 1).clamp(1, 10),
            backup_scan_with_live: host.config_bool("backupScanWithLive", true),
            audio_auto_copy: host.config_str("audioAutoCopy", ""),
            default_channel: host.config_str("defaultChannel", "Canal+ VIP RAW").trim().to_string(),
            backup_enabled_failover: host.config_bool("autoFailover", true),
        }
    }

    pub fn url_of(&self, provider: ProviderId) -> &str {
        match provider {
            ProviderId::Primary => &self.primary_url,
            ProviderId::Backup => &self.backup_url,
        }
    }

    pub fn user_agent_of(&self, provider: ProviderId) -> &str {
        match provider {
            ProviderId::Primary => &self.user_agent,
            ProviderId::Backup => &self.backup_user_agent,
        }
    }

    pub fn portal(&self, provider: ProviderId) -> Option<Portal> {
        let url = self.url_of(provider);
        if url.is_empty() { None } else { Portal::parse(url).ok() }
    }

    pub fn has_backup(&self) -> bool {
        !self.backup_url.is_empty()
    }

    pub fn single_connection(&self) -> Vec<ProviderId> {
        if self.backup_max_connections <= 1 { vec![ProviderId::Backup] } else { Vec::new() }
    }

    pub fn cmd_options(&self) -> Options {
        Options {
            default_user_agent: self.user_agent.clone(),
            single_connection: self.single_connection(),
            audio_default: AudioMode::Aac,
            auto_copy: self.audio_auto_copy.split('|').map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).collect(),
            audio_seen_copyable: false,
        }
    }
}
