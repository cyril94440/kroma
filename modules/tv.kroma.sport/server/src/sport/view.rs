use std::collections::BTreeMap;

use super::event::{SportEvent, State};
use super::memory::{Memory, SeenChannel};
use super::reading::Certainty;
use super::view_types::{
    ChannelLink, ChannelView, EventView, Ligue1View, MultiplexChannel, OnScreen, ScanStatus,
    SportView,
};
use crate::text::fold;

const POST_VISIBLE_S: f64 = 4.0 * 3600.0;
const VIEW_MAX: usize = 60;

fn is_multiplex(seen: &SeenChannel) -> bool {
    let name = fold(seen.name.as_deref().unwrap_or_default());
    name.contains("ligue") && (name.contains("1+") || name.contains("1 +"))
}

fn contradicted(memory: &Memory, link: &ChannelLink, event: &SportEvent) -> bool {
    memory.image.values().any(|s| {
        Some(&s.channel_id) == link.channel_id.as_ref()
            && s.event.as_ref().is_some_and(|e| e != &event.id)
            && s.certainty == Some(Certainty::Seen)
    })
}

fn foreign_too_early(
    memory: &Memory,
    link: &ChannelLink,
    event: &SportEvent,
    scan: &ScanStatus,
) -> bool {
    if !(link.foreign && event.is_ligue1()) {
        return false;
    }
    if event.state != State::In || scan.state == "en_cours" {
        return true;
    }
    if scan.state == "impossible" {
        return false;
    }
    !memory
        .image
        .values()
        .any(|s| is_multiplex(s) && s.at >= event.start - 300.0)
}

fn channel_for(
    event: &SportEvent,
    memory: &Memory,
    links: &BTreeMap<String, ChannelLink>,
    scan: &ScanStatus,
) -> Option<ChannelView> {
    if let Some(seen) = memory.seen_channel(&event.id) {
        return Some(ChannelView {
            name: seen.name.clone().unwrap_or_default(),
            channel_id: Some(seen.channel_id.clone()),
            source: "image",
            certainty: if seen.certainty == Some(Certainty::Seen) {
                "vu"
            } else {
                "a_confirmer"
            },
            vu_ts: Some((seen.at * 1000.0) as i64),
            foreign: false,
        });
    }
    let link = links.get(&event.id)?;
    if contradicted(memory, link, event) || foreign_too_early(memory, link, event, scan) {
        return None;
    }
    let from_guide = link.source == "guide";
    Some(ChannelView {
        name: link.name.clone(),
        channel_id: link.channel_id.clone(),
        source: if from_guide { "guide" } else { "web" },
        certainty: if from_guide { "guide" } else { "a_confirmer" },
        vu_ts: None,
        foreign: link.foreign,
    })
}

pub fn public_view(
    events: &[SportEvent],
    memory: &Memory,
    links: &BTreeMap<String, ChannelLink>,
    on_screen: &OnScreen,
    scan: &ScanStatus,
    now: f64,
) -> SportView {
    let mut ordered: Vec<&SportEvent> = events
        .iter()
        .filter(|e| !(e.state == State::Post && now - e.start > POST_VISIBLE_S))
        .collect();
    ordered.sort_by(|a, b| {
        let key = |e: &SportEvent| {
            (
                e.state.rank(),
                if e.state == State::In {
                    -e.popularity
                } else {
                    0
                },
                if e.state == State::In { 0.0 } else { e.start },
                -e.popularity,
            )
        };
        key(a)
            .partial_cmp(&key(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let views: Vec<EventView> = ordered
        .into_iter()
        .map(|e| {
            let channel = channel_for(e, memory, links, scan);
            let here = channel.as_ref().is_some_and(|c| {
                on_screen.channel_id.is_some() && c.channel_id == on_screen.channel_id
            }) || on_screen.espn.as_ref() == Some(&e.espn);
            EventView {
                id: e.id.clone(),
                sport: e.sport.wire(),
                competition: e.competition.clone(),
                state: e.state.wire(),
                start_ms: (e.start * 1000.0) as i64,
                clock: e.clock.clone(),
                lines: e.lines.clone(),
                channel,
                on_screen: here,
                hide_score: here && e.state != State::Pre,
                ligue1: e.is_ligue1(),
                popularity: e.popularity,
            }
        })
        .collect();
    let mut seen: Vec<&SeenChannel> = memory.image.values().filter(|s| is_multiplex(s)).collect();
    seen.sort_by_key(|s| (s.num.is_none(), s.num.unwrap_or(0)));
    let channels = seen
        .into_iter()
        .map(|s| {
            let event = s
                .event
                .as_ref()
                .and_then(|id| events.iter().find(|e| &e.id == id));
            MultiplexChannel {
                num: s.num,
                name: s.name.clone(),
                channel_id: s.channel_id.clone(),
                state: s.state.clone(),
                event: s.event.clone(),
                certainty: s.certainty,
                teams: event.map_or_else(
                    || s.teams.clone(),
                    |e| e.lines.iter().map(|l| l.name.clone()).collect(),
                ),
                score: event.map_or_else(
                    || s.read_score.clone().unwrap_or_default(),
                    |e| {
                        e.lines
                            .iter()
                            .map(|l| l.score.clone())
                            .collect::<Vec<_>>()
                            .join("-")
                    },
                ),
                clock: event.map(|e| e.clock.clone()).unwrap_or_default(),
                vu_ts: (s.at * 1000.0) as i64,
                on_screen: on_screen.channel_id.as_ref() == Some(&s.channel_id),
            }
        })
        .collect();
    let ligue1_live = views.iter().filter(|e| e.ligue1 && e.state == "in").count();
    let ligue1_next = views
        .iter()
        .find(|e| e.ligue1 && e.state == "pre")
        .map(|e| e.start_ms);
    SportView {
        updated_ms: (now * 1000.0) as i64,
        live: views.iter().filter(|e| e.state == "in").count(),
        next_ms: views.iter().find(|e| e.state == "pre").map(|e| e.start_ms),
        ligue1: Ligue1View {
            live: ligue1_live,
            next_ms: ligue1_next,
            channels,
        },
        events: views.into_iter().take(VIEW_MAX).collect(),
        scan: scan.clone(),
    }
}
