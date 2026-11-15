use std::collections::BTreeMap;

use super::support::{event, live_match, reading, scan_row, NOW};
use crate::sport::{public_view, ChannelLink, Memory, OnScreen, ScanStatus, State};

fn guide_link(name: &str, channel: &str, foreign: bool) -> ChannelLink {
    ChannelLink {
        name: name.into(),
        channel_id: Some(channel.into()),
        source: "guide".into(),
        at: NOW,
        foreign,
    }
}

fn scan(state: &str) -> ScanStatus {
    ScanStatus {
        state: state.into(),
        ..ScanStatus::default()
    }
}

fn channel_of<'a>(
    view: &'a crate::sport::SportView,
    id: &str,
) -> Option<&'a crate::sport::ChannelView> {
    view.events
        .iter()
        .find(|e| e.id == id)
        .unwrap()
        .channel
        .as_ref()
}

#[test]
fn the_image_beats_the_guide_and_the_screen_score_is_hidden() {
    let match_ = live_match();
    let events = std::slice::from_ref(&match_);
    let links = BTreeMap::from([(match_.id.clone(), guide_link("Canal+", "p:9", false))]);
    let mut memory = Memory::default();
    let from_guide = public_view(
        events,
        &memory,
        &links,
        &OnScreen::default(),
        &scan("inactif"),
        NOW,
    );
    memory.note_row(
        &scan_row(
            "p:11",
            "ok",
            Some(reading("Lens", "Lille", "direct", 0.9)),
            1,
        ),
        events,
        NOW,
    );

    let view = public_view(
        events,
        &memory,
        &links,
        &OnScreen {
            channel_id: Some("p:11".into()),
            espn: None,
        },
        &scan("fait"),
        NOW,
    );

    assert_eq!(
        from_guide.events[0].channel.as_ref().unwrap().source,
        "guide"
    );
    assert_eq!(from_guide.live, 1);
    assert_eq!(view.events[0].channel.as_ref().unwrap().source, "image");
    assert!(view.events[0].on_screen && view.events[0].hide_score);
    assert_eq!(
        (
            view.ligue1.channels[0].num,
            view.ligue1.channels[0].score.as_str()
        ),
        (Some(1), "1-0")
    );
    assert!(!serde_json::to_string(&view)
        .unwrap()
        .to_lowercase()
        .contains("http"));
}

#[test]
fn a_guide_channel_the_image_saw_another_match_on_is_dropped() {
    let (a, b) = (
        live_match(),
        event("foot:2", "Brest", "Nantes", State::In, NOW - 1800.0),
    );
    let events = [a.clone(), b.clone()];
    let links = BTreeMap::from([(b.id.clone(), guide_link("Canal+", "p:11", false))]);
    let mut memory = Memory::default();
    memory.note_row(
        &scan_row(
            "p:11",
            "ok",
            Some(reading("Lens", "Lille", "direct", 0.9)),
            1,
        ),
        &events,
        NOW,
    );

    let view = public_view(
        &events,
        &memory,
        &links,
        &OnScreen::default(),
        &ScanStatus::default(),
        NOW,
    );

    assert!(channel_of(&view, "foot:2").is_none());
    assert_eq!(
        view.events
            .iter()
            .find(|e| e.id == "foot:1")
            .unwrap()
            .channel
            .as_ref()
            .unwrap()
            .source,
        "image"
    );
}

#[test]
fn a_foreign_channel_is_the_last_resort_for_a_ligue_1_match() {
    let (live, pre) = (
        live_match(),
        event("foot:3", "Lens", "Lille", State::Pre, NOW + 3600.0),
    );
    let events = [live.clone(), pre.clone()];
    let link = guide_link("HR: ARENA SPORT 3", "p:9", true);
    let links = BTreeMap::from([(live.id.clone(), link.clone()), (pre.id.clone(), link)]);
    let view = |memory: &Memory, state: &str| {
        public_view(
            &events,
            memory,
            &links,
            &OnScreen::default(),
            &scan(state),
            NOW,
        )
    };
    let mut memory = Memory::default();

    let before_scan = view(&memory, "inactif");
    let impossible = view(&memory, "impossible");
    memory.note_row(
        &scan_row(
            "p:12",
            "ok",
            Some(reading("Brest", "Nantes", "direct", 0.9)),
            2,
        ),
        &events,
        NOW,
    );
    let running = view(&memory, "en_cours");
    let done = view(&memory, "fait");

    assert!(
        channel_of(&before_scan, "foot:1").is_none()
            && channel_of(&running, "foot:1").is_none()
            && channel_of(&done, "foot:3").is_none()
    );
    assert!(channel_of(&impossible, "foot:1").unwrap().foreign);
    assert_eq!(
        channel_of(&done, "foot:1").unwrap().name,
        "HR: ARENA SPORT 3"
    );
}

#[test]
fn live_matches_come_first_then_by_start_time() {
    let events = [
        event("foot:1", "A", "B", State::Pre, NOW + 100.0),
        event("foot:2", "C", "D", State::In, NOW - 600.0),
        event("foot:3", "E", "F", State::Pre, NOW + 50.0),
    ];

    let view = public_view(
        &events,
        &Memory::default(),
        &BTreeMap::new(),
        &OnScreen::default(),
        &ScanStatus::default(),
        NOW,
    );

    assert_eq!(
        view.events
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["foot:2", "foot:3", "foot:1"]
    );
}
