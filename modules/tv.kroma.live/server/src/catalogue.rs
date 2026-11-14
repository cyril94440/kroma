use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::model::{Channel, ProviderId};
use crate::norm::{self, Family};
use crate::rules::Rules;
use crate::score::{asks_4k, country_score, expand_query, preference, query_score};

#[derive(Debug, Clone, Default)]
pub struct Catalogue {
    channels: Vec<Channel>,
    by_stream: HashMap<i64, usize>,
    by_epg: HashMap<String, Vec<usize>>,
    families: BTreeMap<Family, FamilyInfo>,
}

#[derive(Debug, Clone, Default)]
struct FamilyInfo {
    numbers: BTreeSet<u32>,
    has_guide: bool,
}

#[derive(Debug, Clone)]
pub struct SlotCandidate {
    pub channel: Channel,
    pub alternatives: Vec<Channel>,
    pub family: Family,
    pub number: Option<u32>,
}

impl Catalogue {
    pub fn new(channels: Vec<Channel>) -> Catalogue {
        let mut by_stream = HashMap::new();
        let mut by_epg: HashMap<String, Vec<usize>> = HashMap::new();
        let mut families: BTreeMap<Family, FamilyInfo> = BTreeMap::new();
        for (at, channel) in channels.iter().enumerate() {
            by_stream.insert(channel.stream_id, at);
            if let Some(epg) = &channel.epg_id {
                by_epg.entry(epg.to_lowercase()).or_default().push(at);
            }
            let (family, number) = channel.slot.clone();
            let info = families.entry(family).or_default();
            info.numbers.extend(number);
            info.has_guide |= channel.epg_id.is_some();
        }
        Catalogue { channels, by_stream, by_epg, families }
    }

    pub fn len(&self) -> usize {
        self.channels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    pub fn all(&self) -> &[Channel] {
        &self.channels
    }

    pub fn by_stream_id(&self, stream_id: i64) -> Option<&Channel> {
        self.by_stream.get(&stream_id).map(|at| &self.channels[*at])
    }

    fn matching(&self, query: &str, allow_4k: bool, compact_match: bool) -> Vec<&Channel> {
        let terms: Vec<&str> = query.split(' ').filter(|t| !t.is_empty()).collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let compacted = norm::compact(query);
        let compact_terms: Vec<&str> = compacted.split(' ').filter(|t| !t.is_empty()).collect();
        self.channels
            .iter()
            .filter(|c| allow_4k || !c.is_4k)
            .filter(|c| {
                terms.iter().all(|t| c.norm.contains(t))
                    || (compact_match && compact_terms.iter().all(|t| c.compact.contains(t)))
            })
            .collect()
    }

    fn candidates(&self, query: &str, rules: &Rules, compact_match: bool) -> (Vec<&Channel>, String) {
        let normalised = norm::fold(query);
        let allow_4k = rules.allow_4k || asks_4k(&normalised);
        let mut found = self.matching(&normalised, allow_4k, compact_match);
        let mut effective = normalised.clone();
        if found.is_empty() {
            let expanded = expand_query(&normalised);
            if expanded != normalised {
                found = self.matching(&expanded, allow_4k, compact_match);
                effective = expanded;
            }
        }
        if found.is_empty() {
            let bare: Vec<&str> = effective.split(' ').filter(|t| !t.is_empty() && !norm::is_tag(t)).collect();
            let bare = bare.join(" ");
            if !bare.is_empty() && bare != effective {
                found = self.matching(&bare, allow_4k, compact_match);
                effective = bare;
            }
        }
        (found, effective)
    }

    pub fn find(&self, query: &str, rules: &Rules) -> Option<&Channel> {
        let (found, effective) = self.candidates(query, rules, false);
        found.into_iter().max_by(|a, b| {
            query_score(a, &effective, rules)
                .partial_cmp(&query_score(b, &effective, rules))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub fn find_all(&self, query: &str, limit: usize, compact_match: bool, rules: &Rules) -> Vec<(&Channel, f64)> {
        let (mut found, effective) = self.candidates(query, rules, compact_match);
        found.sort_by(|a, b| {
            query_score(b, &effective, rules)
                .partial_cmp(&query_score(a, &effective, rules))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(norm::chan_number(&a.norm).cmp(&norm::chan_number(&b.norm)))
                .then(a.norm.cmp(&b.norm))
        });
        found.into_iter().take(limit).map(|c| (c, query_score(c, &effective, rules))).collect()
    }

    pub fn best_for_epg(&self, epg_id: &str, rules: &Rules) -> Option<&Channel> {
        self.by_epg
            .get(&epg_id.to_lowercase())?
            .iter()
            .map(|at| &self.channels[*at])
            .filter(|c| !c.is_4k)
            .max_by(|a, b| query_score(a, "", rules).partial_cmp(&query_score(b, "", rules)).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn equivalents(&self, name: &str, source: Option<&Catalogue>, limit: usize, rules: &Rules) -> Vec<&Channel> {
        let normalised = norm::fold(name);
        let key = norm::channel_key(&normalised);
        let allow_4k = rules.allow_4k || asks_4k(&normalised);
        let acceptable = |c: &&Channel| allow_4k || !c.is_4k;
        let mut found: Vec<&Channel> = self.channels.iter().filter(|c| c.key == key).filter(acceptable).collect();
        if found.is_empty() && key.words.contains("1") {
            let mut without = key.clone();
            without.words.remove("1");
            found = self.channels.iter().filter(|c| c.key == without).filter(acceptable).collect();
        }
        if found.is_empty() {
            if let Some(source) = source {
                found = self.through_guide(&key, source, allow_4k);
            }
        }
        found.sort_by(|a, b| query_score(b, "", rules).partial_cmp(&query_score(a, "", rules)).unwrap_or(std::cmp::Ordering::Equal));
        found.truncate(limit);
        found
    }

    fn through_guide(&self, key: &norm::ChannelKey, source: &Catalogue, allow_4k: bool) -> Vec<&Channel> {
        let ids: HashSet<String> = source
            .channels
            .iter()
            .filter(|c| &c.key == key)
            .filter_map(|c| c.epg_id.as_ref().map(|id| id.to_lowercase()))
            .collect();
        let numbers: BTreeSet<&str> = key.words.iter().filter(|w| w.bytes().all(|b| b.is_ascii_digit())).map(String::as_str).collect();
        let timeshift = key.words.contains("+1");
        let guided: Vec<&Channel> = self
            .channels
            .iter()
            .filter(|c| c.epg_id.as_ref().is_some_and(|id| ids.contains(&id.to_lowercase())))
            .filter(|c| allow_4k || !c.is_4k)
            .filter(|c| {
                let theirs = c.key_numbers();
                (numbers.is_empty() || theirs.is_empty() || theirs == numbers) && c.key.words.contains("+1") == timeshift
            })
            .collect();
        let keys: HashSet<&norm::ChannelKey> = guided.iter().map(|c| &c.key).collect();
        self.channels.iter().filter(|c| keys.contains(&c.key) && (allow_4k || !c.is_4k)).collect()
    }

    pub fn is_event_channel(&self, name: &str) -> bool {
        let (family, _) = norm::family_slot(&norm::fold(name));
        if ["ppv", "match", "event"].iter().any(|word| family.head.split(' ').any(|w| w == *word)) {
            return true;
        }
        self.families.get(&family).is_some_and(|info| info.numbers.len() >= 3 && !info.has_guide)
    }

    pub fn slot_candidates(&self, query: &str, limit: usize, first: Option<u32>, rules: &Rules) -> Vec<SlotCandidate> {
        let (found, effective) = self.candidates(query, rules, true);
        let mut slots: BTreeMap<(Family, Option<u32>), Vec<&Channel>> = BTreeMap::new();
        for channel in found {
            let (family, number) = channel.slot.clone();
            let number = self.slot_number(&family, number);
            slots.entry((family, number)).or_default().push(channel);
        }
        let mut best: HashMap<Family, f64> = HashMap::new();
        let mut out: Vec<SlotCandidate> = Vec::new();
        for ((family, number), mut members) in slots {
            members.sort_by(|a, b| {
                query_score(b, &effective, rules)
                    .partial_cmp(&query_score(a, &effective, rules))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.norm.cmp(&b.norm))
            });
            let top = query_score(members[0], &effective, rules);
            let entry = best.entry(family.clone()).or_insert(f64::MIN);
            *entry = entry.max(top);
            out.push(SlotCandidate {
                channel: members[0].clone(),
                alternatives: members[1..].iter().map(|c| (*c).clone()).collect(),
                family,
                number,
            });
        }
        out.sort_by(|a, b| {
            let first_a = first.is_some() && a.number != first;
            let first_b = first.is_some() && b.number != first;
            first_a
                .cmp(&first_b)
                .then(best[&b.family].partial_cmp(&best[&a.family]).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.family.cmp(&b.family))
                .then(a.number.is_none().cmp(&b.number.is_none()))
                .then(a.number.unwrap_or(0).cmp(&b.number.unwrap_or(0)))
        });
        out.truncate(limit.max(1));
        out
    }

    fn slot_number(&self, family: &Family, number: Option<u32>) -> Option<u32> {
        if number.is_none() {
            if let Some(info) = self.families.get(family) {
                if !info.numbers.is_empty() && !info.numbers.contains(&1) {
                    return Some(1);
                }
            }
        }
        number
    }

    pub fn suggest(&self, query: &str, limit: usize, rules: &Rules) -> Vec<&Channel> {
        let normalised = norm::fold(query);
        let allow_4k = rules.allow_4k || asks_4k(&normalised);
        let hits = |text: &str| -> Vec<(&Channel, usize)> {
            let words: Vec<&str> = text.split(' ').filter(|t| !t.is_empty()).collect();
            let strong: Vec<&str> = {
                let kept: Vec<&str> = words.iter().copied().filter(|t| t.chars().count() >= 3 && !t.bytes().all(|b| b.is_ascii_digit())).collect();
                if kept.is_empty() { words.clone() } else { kept }
            };
            self.channels
                .iter()
                .filter(|c| allow_4k || !c.is_4k)
                .map(|c| (c, strong.iter().filter(|t| c.norm.contains(**t)).count()))
                .filter(|(_, count)| *count > 0)
                .collect()
        };
        let mut found = hits(&normalised);
        if found.is_empty() {
            found = hits(&expand_query(&normalised));
        }
        found.sort_by(|a, b| {
            b.1.cmp(&a.1).then(preference(&b.0.norm, rules).partial_cmp(&preference(&a.0.norm, rules)).unwrap_or(std::cmp::Ordering::Equal))
        });
        found.into_iter().take(limit).map(|(c, _)| c).collect()
    }

    pub fn country_tier(&self, name: &str, rules: &Rules) -> f64 {
        country_score(&norm::fold(name), rules)
    }

    pub fn provider(&self) -> Option<ProviderId> {
        self.channels.first().map(|c| c.provider)
    }
}
