use super::Sport;
use crate::peers::LiveStatus;
use crate::prono::DirectKey;

const STATUS_EVERY_S: f64 = 5.0;

impl Sport {
    pub(crate) fn step_status(&self, now: f64) {
        {
            let st = self.lock();
            if now - st.presence.at < STATUS_EVERY_S {
                return;
            }
        }
        let status = self.live.status();
        let mut st = self.lock();
        st.presence.at = now;
        match status {
            Ok(status) => self.apply_status(&mut st, status),
            Err(_) => st.presence.known = false,
        }
    }

    fn apply_status(&self, st: &mut super::State, status: LiveStatus) {
        let channel_changed = st.presence.channel.as_ref().map(|c| &c.id)
            != status.channel.as_ref().map(|c| &c.id)
            || st.presence.provider != status.provider;
        st.presence.known = true;
        st.presence.viewers = status.viewers;
        st.presence.channel = status.channel;
        st.presence.provider = status.provider;
        st.presence.started_at = status.started_at;
        if channel_changed {
            st.detect.force = true;
        }
        st.pronos.direct_key = direct_key(&st.presence);
    }

    pub(crate) fn watched(&self) -> bool {
        let st = self.lock();
        !st.presence.known || !st.presence.viewers.is_empty()
    }
}

pub(crate) fn direct_key(presence: &super::state::Presence) -> Option<DirectKey> {
    let channel = presence.channel.as_ref()?;
    Some(vec![
        presence.provider.clone(),
        channel.id.clone(),
        presence
            .started_at
            .map(|s| s.to_string())
            .unwrap_or_default(),
    ])
}
