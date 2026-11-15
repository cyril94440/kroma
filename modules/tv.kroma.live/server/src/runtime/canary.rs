use super::Live;
use crate::db::{self, ProviderEvent};
use crate::model::{Channel, ProviderId};

impl Live {
    pub(crate) async fn canary_alive(&self, skip: Option<&Channel>) -> bool {
        let (settings, rules) = (self.settings(), self.rules());
        if settings.canaries.is_empty() {
            return false;
        }
        let primary = self.providers.catalogue(ProviderId::Primary);
        let candidates: Vec<Channel> = settings.canaries.iter().filter_map(|name| primary.find(name, &rules).cloned()).filter(|c| skip.is_none_or(|s| s.id() != c.id())).collect();
        if candidates.is_empty() || !self.providers.ping(ProviderId::Primary, &settings) {
            return false;
        }
        let first = self.canary_probe(&candidates[0]);
        let second = async {
            match candidates.get(1) {
                Some(channel) => self.canary_probe(channel).await,
                None => false,
            }
        };
        let (first, second) = tokio::join!(first, second);
        first || second
    }

    async fn canary_probe(&self, channel: &Channel) -> bool {
        let probed = self.quick_probe(channel).await;
        if !probed.alive && !probed.busy {
            let detail = probed.reason.clone().unwrap_or_default();
            let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: super::now_ms(), kind: "canaryDead".into(), channel: Some(channel.name.clone()), detail, ..ProviderEvent::default() });
        }
        probed.alive
    }
}
