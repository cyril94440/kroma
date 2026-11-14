use super::support::*;

#[test]
fn the_first_reading_seeds_the_room_and_a_real_arrival_is_announced_once() {
    let rig = rig();
    rig.present(&["Cyril"]);
    rig.party.refresh_presence();
    assert!(rig.keys().is_empty());
    rig.present(&["Cyril", "Mamie"]);
    rig.party.refresh_presence();
    assert_eq!(rig.keys(), vec!["party.joined"]);
    rig.party.refresh_presence();
    assert_eq!(rig.keys().len(), 1);
}

#[test]
fn a_departure_is_announced_only_after_the_grace_and_a_return_cancels_it() {
    let rig = rig();
    rig.present(&["Cyril", "Mamie"]);
    rig.party.refresh_presence();
    rig.present(&["Cyril"]);
    rig.advance(60_000);
    rig.party.refresh_presence();
    rig.present(&["Cyril", "Mamie"]);
    rig.advance(10_000);
    rig.party.refresh_presence();
    assert!(rig.keys().is_empty());
    rig.present(&["Cyril"]);
    rig.party.refresh_presence();
    rig.advance(181_000);
    rig.party.refresh_presence();
    assert_eq!(rig.keys(), vec!["party.left"]);
}

#[test]
fn an_unreachable_live_module_changes_nothing() {
    let rig = rig();
    rig.present(&["Cyril"]);
    rig.party.refresh_presence();
    rig.unreachable();
    rig.party.refresh_presence();
    assert!(rig.keys().is_empty());
    assert_eq!(rig.party.presence().len(), 1);
}

#[test]
fn a_user_on_two_devices_is_one_viewer() {
    let rig = rig();
    *rig.live.present.lock().unwrap() = Some(vec![
        crate::live::Viewer { user_id: "u1".into(), name: "Ana".into(), since: 1 },
        crate::live::Viewer { user_id: "u1".into(), name: "Ana".into(), since: 2 },
    ]);
    assert_eq!(rig.party.fetch_presence().unwrap().len(), 1);
}

#[test]
fn a_sustained_instability_is_announced_once_per_incident_window() {
    let rig = rig();
    rig.present(&["Cyril"]);
    rig.party.refresh_presence();
    rig.live.status.lock().unwrap().health.stutter = true;
    rig.live.status.lock().unwrap().channel = Some(crate::live::ChannelRef { id: "tf1".into(), name: "TF1".into() });
    for step in 0..30 {
        rig.advance(15_000);
        rig.party.tick();
        let _ = step;
    }
    let alerts = rig.keys().iter().filter(|k| *k == "party.alert.unstable").count();
    assert_eq!(alerts, 1, "{:?}", rig.keys());
    for _ in 0..70 {
        rig.advance(15_000);
        rig.party.tick();
    }
    assert!(rig.keys().iter().filter(|k| *k == "party.alert.unstable").count() >= 2);
    rig.live.status.lock().unwrap().health.stutter = false;
    rig.advance(15_000);
    rig.party.tick();
    assert!(rig.party.lock().alert.unstable_since.is_none());
}

#[test]
fn a_short_blip_or_an_empty_room_raises_no_alert() {
    let rig = rig();
    rig.present(&["Cyril"]);
    rig.party.refresh_presence();
    rig.live.status.lock().unwrap().health.black = true;
    rig.advance(15_000);
    rig.party.tick();
    rig.live.status.lock().unwrap().health.black = false;
    rig.advance(15_000);
    rig.party.tick();
    assert!(rig.keys().is_empty());

    let rig = self::rig();
    rig.present(&[]);
    rig.party.refresh_presence();
    rig.live.status.lock().unwrap().health.frozen = true;
    for _ in 0..30 {
        rig.advance(15_000);
        rig.party.tick();
    }
    assert!(rig.keys().is_empty());
}
