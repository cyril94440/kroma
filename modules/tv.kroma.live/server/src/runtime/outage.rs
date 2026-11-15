use std::sync::Arc;

use serde_json::json;

use super::notify::CHANNEL_DOWN;
use super::snapshot::TOPIC_FAILOVER;
use super::switch::SwitchRequest;
use super::{now_ms, Live};
use crate::db::{self, ProviderEvent};
use crate::model::{Channel, ProviderId};
use crate::status::Phase;

const MAX_VARIANTS_PROBED: usize = 4;
const FAMILY_LIMIT: usize = 12;
const EQUIVALENTS_PROBED: usize = 3;

impl Live {
    pub(crate) fn still_live(&self, channel: &Channel) -> bool {
        let inner = self.lock();
        inner.current.as_ref().is_some_and(|c| c.id() == channel.id()) && inner.phase != Phase::Paused
    }

    pub(crate) async fn working_equivalent(&self, channel: &Channel, to: ProviderId) -> Result<Channel, Vec<String>> {
        let rules = self.rules();
        let mut busy = Vec::new();
        let mut tried = Vec::new();
        for candidate in self.providers.equivalents(channel, to, EQUIVALENTS_PROBED, &rules) {
            let probed = self.quick_probe(&candidate).await;
            if probed.busy {
                busy.push(candidate);
                continue;
            }
            if probed.alive {
                return Ok(candidate);
            }
            tried.push(probed.reason.unwrap_or_else(|| "noVideo".into()));
            let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "equivalentDead".into(), channel: Some(candidate.name), to: Some(to.as_str().into()), detail: tried.last().cloned().unwrap_or_default(), ..ProviderEvent::default() });
        }
        for candidate in busy {
            let probed = self.quick_probe(&candidate).await;
            if probed.alive {
                return Ok(candidate);
            }
            tried.push(probed.reason.unwrap_or_else(|| "busy".into()));
        }
        Err(tried)
    }

    pub(crate) async fn channel_down_locally(self: &Arc<Self>, current: &Channel, now: f64, began: Option<f64>) {
        let rules = self.rules();
        let variants = self.providers.equivalents(current, ProviderId::Primary, FAMILY_LIMIT, &rules);
        let family: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
        let (left, muted) = self.lock().policy.variant_state(now, &current.name, &family);
        let mut found = None;
        if left > 0 {
            for variant in variants.iter().filter(|v| v.id() != current.id() && !muted.contains(&v.name)).take(MAX_VARIANTS_PROBED) {
                if self.quick_probe(variant).await.alive {
                    found = Some(variant.clone());
                    break;
                }
            }
        }
        if !self.still_live(current) {
            return;
        }
        let (returned, speak, tried) = {
            let mut inner = self.lock();
            let returned = inner.policy.media_returned(began);
            let mut speak = true;
            if !returned {
                match &found {
                    Some(variant) => inner.policy.did_variant_switch(now, &current.name, &variant.name, &family),
                    None => {
                        inner.policy.did_channel_down(now, Some(&current.name), Some(&family));
                        speak = inner.policy.claim_family_alert(now);
                    }
                }
            }
            (returned, speak, inner.policy.variants_tried())
        };
        if returned {
            return;
        }
        let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "channelDown".into(), channel: Some(current.name.clone()), detail: found.as_ref().map(|v| v.name.clone()).unwrap_or_default(), ..ProviderEvent::default() });
        if found.is_none() && !speak {
            return;
        }
        let backup = self.settings().has_backup();
        self.emit(TOPIC_FAILOVER, json!({ "kind": "channelDown", "channel": current.name, "variant": found.as_ref().map(|v| v.name.clone()), "budgetSpent": left == 0, "familyTried": tried, "backupAvailable": backup }));
        self.notify_people(CHANNEL_DOWN, &current.name, found.as_ref().map(|v| v.name.as_str()));
        if let Some(variant) = found {
            self.switch(SwitchRequest { channel_id: variant.id(), fast: true, reason: "channelDown".into(), ..SwitchRequest::default() }).await;
        }
    }
}
