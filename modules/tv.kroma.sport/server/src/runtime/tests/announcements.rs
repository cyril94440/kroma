use super::support::{at, villarreal_summary, Rig};

#[test]
fn a_goal_is_announced_after_the_delay_with_a_card_and_its_clip_is_cut() {
    let rig = Rig::new("2026-11-03T20:11:00Z");
    rig.serve_summary(&villarreal_summary());

    let early = rig.run_until(20, |r| !r.room.texts().is_empty());
    let announced = rig.run_until(150, |r| !r.room.texts().is_empty());
    let cut = rig.run_until(60, |r| !r.live.cuts.lock().unwrap().is_empty());

    let said = rig.room.said.lock().unwrap().clone();
    assert!(!early && announced && cut);
    assert!(
        said[0]
            .0
            .starts_with("But ! Villarreal 0 - 1 Paris Saint-Germain, 9'"),
        "{}",
        said[0].0
    );
    assert_eq!(said[0].1.as_ref().unwrap()["kind"], "match");
    assert_eq!(said.len(), 1);
    let cuts = rig.live.cuts.lock().unwrap().clone();
    assert_eq!(cuts.len(), 1);
    assert!(cuts[0].2.starts_with("espn-401915309-but-"));
    assert_eq!(cuts[0].1 - cuts[0].0, (95 + 70) * 1000);
    let clips = rig.sport.clips(None, 10);
    assert_eq!(
        (
            clips.len(),
            clips[0]["score"].as_str(),
            clips[0]["minute"].as_str()
        ),
        (1, Some("0-1"), Some("9'"))
    );
    assert_eq!(clips[0]["file"]["module"], "tv.kroma.live");
    assert!(rig
        .events("sport.clips")
        .iter()
        .any(|e| e["clips"].as_array().is_some_and(|c| c.len() == 1)));
}

#[test]
fn a_goal_already_old_when_first_seen_is_marked_without_a_message_and_never_repeats() {
    let rig = Rig::new("2026-11-03T20:20:00Z");
    rig.serve_summary(&villarreal_summary());

    rig.run(200);

    assert!(rig.room.texts().is_empty());
    assert_eq!(
        rig.sport.snapshot("u-ana", "Ana", false)["match"]["evenements"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == "but")
            .count(),
        1
    );
}

#[test]
fn a_clock_reading_anchors_the_period_on_the_image_and_reveals_the_goal_at_once() {
    let rig = Rig::new("2026-11-03T20:11:00Z");
    rig.serve_summary(&villarreal_summary());
    rig.run_until(60, |r| !r.events("sport.match").is_empty());
    let captured_ms = (at("2026-11-03T20:11:30Z") * 1000.0) as i64;

    let refused = rig
        .sport
        .calibrate("not a clock", Some(captured_ms))
        .unwrap_err();
    let wrong = rig.sport.calibrate("75:00", Some(captured_ms)).unwrap_err();
    let done = rig.sport.calibrate("10:05", Some(captured_ms)).unwrap();
    rig.run_until(60, |r| {
        r.events("sport.match")
            .iter()
            .any(|e| e["match"]["periodes"][0]["t_image"].is_number())
    });

    let latest = rig.events("sport.match").pop().unwrap();
    let k = captured_ms - 605_000;
    assert_eq!(
        (refused.code(), wrong.code()),
        ("unreadable", "implausible")
    );
    assert_eq!(done["periodAnchored"], 1);
    assert_eq!(latest["match"]["periodes"][0]["t_image"], k);
    let goal = latest["match"]["evenements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "but")
        .unwrap();
    assert_eq!(goal["t_image"], k + 508_000 + 3000);
    assert_eq!(rig.room.texts().len(), 1);
}
