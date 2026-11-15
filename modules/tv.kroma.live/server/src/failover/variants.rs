use std::collections::BTreeSet;

use super::*;

impl FailoverPolicy {
    pub fn dead_since(&self) -> Option<f64> {
        self.dead_since
    }

    pub fn unstable_since(&self) -> Option<f64> {
        self.unstable_since
    }

    pub fn media_returned(&self, began_at: Option<f64>) -> bool {
        began_at.is_some() && self.dead_since.is_none()
    }

    pub fn did_channel_down(&mut self, now: f64, channel: Option<&str>, family: Option<&[String]>) {
        self.saved.channel_down = Some(ChannelDown { channel: channel.map(str::to_string), at: now });
        if let (Some(channel), Some(family)) = (channel, family) {
            self.family_of(now, channel, family).names.insert(channel.to_string(), now);
        }
    }

    fn family_of(&mut self, now: f64, channel: &str, family: &[String]) -> &mut Family {
        let names: BTreeSet<&str> = family.iter().map(String::as_str).chain([channel]).collect();
        if let Some(current) = self.saved.variant_family.as_mut() {
            current.switches.retain(|at| now - at < VARIANT_WINDOW);
            current.names.retain(|_, at| now - *at < VARIANT_WINDOW);
            let related = current.names.keys().any(|name| names.contains(name.as_str())) || current.to.as_deref().is_some_and(|to| names.contains(to));
            if !related {
                self.saved.variant_family = None;
            }
        }
        self.saved.variant_family.get_or_insert_with(Family::default)
    }

    pub fn variant_state(&self, now: f64, channel: &str, family: &[String]) -> (usize, BTreeSet<String>) {
        let names: BTreeSet<&str> = family.iter().map(String::as_str).chain([channel]).collect();
        let Some(current) = self.saved.variant_family.as_ref() else {
            return (VARIANT_MAX_SWITCHES, BTreeSet::new());
        };
        let related = current.names.keys().any(|name| names.contains(name.as_str())) || current.to.as_deref().is_some_and(|to| names.contains(to));
        if !related {
            return (VARIANT_MAX_SWITCHES, BTreeSet::new());
        }
        let muted = current.names.iter().filter(|(_, at)| now - **at < AUTO_COOLDOWN).map(|(name, _)| name.clone()).collect();
        let used = current.switches.iter().filter(|at| now - **at < VARIANT_WINDOW).count();
        (VARIANT_MAX_SWITCHES.saturating_sub(used), muted)
    }

    pub fn did_variant_switch(&mut self, now: f64, channel: &str, to: &str, family: &[String]) {
        self.did_channel_down(now, Some(channel), Some(family));
        let current = self.family_of(now, channel, family);
        current.switches.push(now);
        current.to = Some(to.to_string());
    }

    pub fn claim_family_alert(&mut self, now: f64) -> bool {
        let Some(current) = self.saved.variant_family.as_mut() else { return true };
        if current.alerted_at.is_some_and(|at| now - at < AUTO_COOLDOWN) {
            return false;
        }
        current.alerted_at = Some(now);
        true
    }

    pub fn variants_tried(&self) -> bool {
        self.saved.variant_family.as_ref().is_some_and(|family| !family.switches.is_empty())
    }

    pub(super) fn channel_muted(&self, channel: &Option<String>, now: f64) -> bool {
        if let (Some(family), Some(channel)) = (self.saved.variant_family.as_ref(), channel) {
            if family.names.get(channel).is_some_and(|at| now - at < AUTO_COOLDOWN) {
                return true;
            }
        }
        self.saved.channel_down.as_ref().is_some_and(|down| now - down.at < AUTO_COOLDOWN && (down.channel.is_none() || down.channel == *channel))
    }

    pub(super) fn end_variant_incident(&mut self, now: f64, provider: ProviderId, channel: &Option<String>) {
        let Some(family) = self.saved.variant_family.as_ref() else { return };
        let steady = provider == ProviderId::Primary && self.media_since.is_some_and(|since| now - since >= VARIANT_STABLE);
        let inside = channel.as_ref().is_none_or(|name| family.names.contains_key(name) || family.to.as_ref() == Some(name));
        if steady && inside {
            self.saved.variant_family = None;
        }
    }

    pub fn undo(&mut self, action: Action) {
        match action {
            Action::Recovered => self.saved.both_down_alerted = true,
            Action::ReturnIdle => self.last_return_try = None,
            _ => {}
        }
    }
}
