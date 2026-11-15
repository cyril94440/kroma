use kroma_module_sdk::domain::Permission;
use kroma_module_sdk::host::{Audience, NotificationSpec, NotificationSubject};

use super::Live;

pub const FAILOVER: &str = "tv.kroma.live/failover";
pub const FAILBACK: &str = "tv.kroma.live/failback";
pub const BOTH_DOWN: &str = "tv.kroma.live/both-down";
pub const RECOVERED: &str = "tv.kroma.live/recovered";
pub const CHANNEL_DOWN: &str = "tv.kroma.live/channel-down";
pub const COVERAGE_GAP: &str = "tv.kroma.live/coverage-gap";
const SUBJECT: &str = "tv.kroma.live/channel";

pub fn spec(kind: &'static str, channel: &str, detail: Option<&str>) -> NotificationSpec {
    let mut spec = NotificationSpec::declared(kind).param("channel", channel).subject(NotificationSubject::new(SUBJECT, channel)).dedupe_key(format!("{kind}:{channel}"));
    if let Some(detail) = detail {
        spec = spec.param("detail", detail);
    }
    spec
}

pub fn audience_of(kind: &str) -> Audience {
    if kind == COVERAGE_GAP { Audience::permission(Permission::SettingsManage) } else { Audience::permission(Permission::Playback) }
}

impl Live {
    pub(crate) fn notify_people(&self, kind: &'static str, channel: &str, detail: Option<&str>) {
        self.host.notify(&audience_of(kind), &spec(kind, channel, detail));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = include_str!("../../../locales/en.json");
    const FR: &str = include_str!("../../../locales/fr.json");

    #[test]
    fn every_declared_type_has_its_wording_in_both_catalogs_and_its_manifest_entry() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../../../module.json")).unwrap();
        let declared: Vec<&str> = manifest["notifications"].as_array().unwrap().iter().map(|n| n["type"].as_str().unwrap()).collect();

        for kind in [FAILOVER, FAILBACK, BOTH_DOWN, RECOVERED, CHANNEL_DOWN, COVERAGE_GAP] {
            assert!(declared.contains(&kind), "{kind}");
        }
        for catalog in [EN, FR] {
            let words: serde_json::Value = serde_json::from_str(catalog).unwrap();
            for entry in manifest["notifications"].as_array().unwrap() {
                for key in ["labelKey", "titleKey", "bodyKey"] {
                    let name = entry[key].as_str().unwrap();
                    assert!(words.get(name).is_some(), "{name}");
                }
            }
        }
    }

    #[test]
    fn a_notice_names_its_channel_once_and_folds_on_the_channel() {
        let json = serde_json::to_value(spec(FAILOVER, "TF1", Some("deadPrimary"))).unwrap();

        assert_eq!(json["type"], FAILOVER);
        assert_eq!(json["params"]["channel"]["value"], "TF1");
        assert_eq!(json["subject"], serde_json::json!({ "kind": SUBJECT, "id": "TF1" }));
        assert_eq!(json["dedupeKey"], "tv.kroma.live/failover:TF1");
    }

    #[test]
    fn the_coverage_gap_goes_to_administrators_and_the_rest_to_viewers() {
        assert_eq!(audience_of(COVERAGE_GAP), Audience::permission(Permission::SettingsManage));
        assert_eq!(audience_of(BOTH_DOWN), Audience::permission(Permission::Playback));
    }
}
