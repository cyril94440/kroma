use serde::{Deserialize, Serialize};

use crate::db;
use crate::epg_search;
use crate::model::{Channel, Programme, ProviderId};
use crate::runtime::{now_ms, now_next, Live};
use crate::status::Slot;

pub const MAX_PER_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChannelView {
    pub id: String,
    pub name: String,
    pub group: String,
    pub provider: Option<ProviderId>,
    pub score: f64,
    pub now: Option<Slot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ChannelPage {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub channels: Vec<ChannelView>,
}

#[derive(Debug, Default, Clone)]
pub struct ChannelQuery {
    pub q: String,
    pub group: String,
    pub provider: Option<ProviderId>,
    pub page: usize,
    pub per_page: usize,
}

impl Live {
    fn view(&self, channel: &Channel, score: f64, with_now: bool) -> ChannelView {
        let now = with_now.then(|| now_next(self.pool(), channel.epg_id.as_deref(), &channel.name, now_ms()).0).flatten();
        ChannelView { id: channel.id(), name: channel.name.clone(), group: channel.group.clone(), provider: Some(channel.provider), score, now: now.as_ref().map(Slot::of) }
    }

    pub fn list_channels(&self, query: &ChannelQuery) -> ChannelPage {
        let rules = self.rules();
        let provider = query.provider.unwrap_or(ProviderId::Primary);
        let catalogue = self.providers.catalogue(provider);
        let per_page = query.per_page.clamp(1, MAX_PER_PAGE);
        let page = query.page.max(1);
        let mut ranked: Vec<(&Channel, f64)> = if query.q.trim().is_empty() {
            let mut all: Vec<&Channel> = catalogue.all().iter().filter(|c| rules.allow_4k || !c.is_4k).collect();
            all.sort_by(|a, b| a.norm.cmp(&b.norm));
            all.into_iter().map(|c| (c, 0.0)).collect()
        } else {
            catalogue.find_all(&query.q, 5000, true, &rules)
        };
        let group = query.group.trim().to_lowercase();
        if !group.is_empty() {
            ranked.retain(|(c, _)| c.group.to_lowercase() == group);
        }
        let total = ranked.len();
        let channels = ranked.into_iter().skip((page - 1) * per_page).take(per_page).map(|(c, score)| self.view(c, score, true)).collect();
        ChannelPage { total, page, per_page, channels }
    }

    pub fn groups(&self, provider: ProviderId) -> Vec<String> {
        let mut groups: Vec<String> = self.providers.catalogue(provider).all().iter().map(|c| c.group.clone()).filter(|g| !g.is_empty()).collect();
        groups.sort();
        groups.dedup();
        groups
    }

    pub fn favourites(&self) -> Vec<ChannelView> {
        let rules = self.rules();
        let catalogue = self.providers.catalogue(ProviderId::Primary);
        rules.favourites.iter().filter_map(|name| catalogue.find(name, &rules)).map(|c| self.view(c, 0.0, true)).collect()
    }

    pub fn resolve(&self, query: &str, limit: usize) -> Vec<ChannelView> {
        let rules = self.rules();
        let catalogue = self.providers.catalogue(ProviderId::Primary);
        catalogue.find_all(query, limit.clamp(1, MAX_PER_PAGE), true, &rules).into_iter().map(|(c, score)| self.view(c, score, true)).collect()
    }

    fn channel_for_guide(&self, epg_id: &str) -> Option<Channel> {
        let rules = self.rules();
        self.providers.catalogue(ProviderId::Primary).best_for_epg(epg_id, &rules).cloned()
    }

    pub fn guide(&self, query: Option<&str>, from_ms: Option<i64>, to_ms: Option<i64>, limit: usize) -> Vec<Programme> {
        let now = now_ms();
        let (from, to) = (from_ms.unwrap_or(now) / 1000, to_ms.unwrap_or(now + 6 * 3600 * 1000) / 1000);
        let limit = limit.clamp(1, 200);
        let rows = match query.map(str::trim).filter(|q| !q.is_empty()) {
            Some(text) => {
                let tokens = epg_search::tokens(text);
                let mut candidates = db::search_programmes(self.pool(), &tokens, false, from, 2000).unwrap_or_default();
                if candidates.is_empty() && tokens.len() >= 2 {
                    candidates = db::search_programmes(self.pool(), &tokens, true, from, 2000).unwrap_or_default();
                }
                epg_search::rank(candidates, &tokens, now / 1000, limit * 3).into_iter().map(|h| h.programme).collect::<Vec<_>>()
            }
            None => db::programmes_window(self.pool(), from, to, limit * 3).unwrap_or_default(),
        };
        rows.into_iter()
            .filter_map(|row| {
                let channel = self.channel_for_guide(&row.epg_id)?;
                Some(crate::runtime::to_programme(row, &channel.name).with_channel(&channel.id()))
            })
            .take(limit)
            .collect()
    }

    pub fn grid(&self, channel_ids: &[String], from_ms: i64, to_ms: i64) -> Vec<Programme> {
        channel_ids
            .iter()
            .filter_map(|id| self.providers.find_by_ref(id))
            .flat_map(|channel| {
                let rows = channel.epg_id.as_deref().map(|e| db::programmes_of(self.pool(), e, from_ms / 1000, to_ms / 1000).unwrap_or_default()).unwrap_or_default();
                rows.into_iter().map(move |row| crate::runtime::to_programme(row, &channel.name).with_channel(&channel.id()))
            })
            .collect()
    }
}
