use super::support::{event, live_match, reading, NOW};
use crate::sport::{
    channel_in_text, choose_programme, guide_queries, Certainty, GuideProgramme, State,
};

fn programme(title: &str, start: f64, end: f64, channel: &str) -> GuideProgramme {
    GuideProgramme {
        channel_id: channel.into(),
        channel_name: channel.into(),
        title: title.into(),
        start_s: start,
        end_s: end,
        ..GuideProgramme::default()
    }
}

fn upcoming() -> crate::sport::SportEvent {
    event("foot:5", "Sevilla", "Alavés", State::Pre, NOW + 3600.0)
}

fn fits(p: &GuideProgramme) -> bool {
    choose_programme(std::slice::from_ref(p), &upcoming(), NOW, |_| 1.0).is_some()
}

#[test]
fn guide_needs_the_right_title_at_the_right_time() {
    let start = upcoming().start;
    let good = programme("Séville / Alavés", start - 600.0, start + 7000.0, "c");

    assert!(fits(&good));
    assert!(fits(&programme(
        "A venir : Séville / Alavés",
        start - 7000.0,
        start,
        "c"
    )));
    assert!(!fits(&programme(
        "A venir : Séville / Alavés",
        start - 14_000.0,
        start - 7200.0,
        "c"
    )));
    assert!(!fits(&programme(
        "Terminé : Séville / Alavés",
        start - 600.0,
        start + 7000.0,
        "c"
    )));
    assert!(!fits(&programme(
        "Séville / Alavés",
        start + 86_400.0,
        start + 90_000.0,
        "c"
    )));
    assert!(!fits(&programme(
        "Séville, la ville",
        start - 600.0,
        start + 7000.0,
        "c"
    )));
}

#[test]
fn queries_use_the_french_spelling_and_titles_cover_unusual_letters() {
    assert!(guide_queries(&upcoming())[0]
        .to_lowercase()
        .contains("seville"));
    assert!(crate::sport::title_covers(
        "Monaco / FC Nordsjælland",
        "AS Monaco",
        "FC Nordsjælland"
    ));
    assert!(crate::sport::title_covers(
        "Lillestrøm / Viktoria Plzen",
        "Lillestrom",
        "Viktoria Plzen"
    ));
}

#[test]
fn the_guide_prefers_the_french_channel() {
    let start = upcoming().start;
    let event = event("foot:6", "Lens", "Lille", State::Pre, start);
    let both = [
        programme("Lens / Lille", start - 60.0, start + 7000.0, "skyde"),
        programme("Lens / Lille", start - 60.0, start + 7000.0, "canal"),
    ];

    let choice = choose_programme(
        &both,
        &event,
        NOW,
        |name| if name == "canal" { 10.0 } else { 1.0 },
    )
    .unwrap();

    assert_eq!(choice.channel_name, "canal");
}

#[test]
fn the_code_reads_the_channel_in_a_web_answer() {
    assert_eq!(
        channel_in_text("Le match est diffusé sur Canal+ Foot à 21h.").as_deref(),
        Some("canal+ foot")
    );
    assert_eq!(
        channel_in_text("Diffusion sur beIN Sports 1 et en clair").as_deref(),
        Some("bein sports 1")
    );
    assert_eq!(channel_in_text("Aucune information"), None);
    assert_eq!(
        channel_in_text("Diffusé sur Canal+ Live 7.").as_deref(),
        Some("canal+ live 7")
    );
    assert_eq!(channel_in_text("Diffusé sur Canal+ Live."), None);
}

#[test]
fn an_image_reading_names_the_live_match_and_how_sure_it_is() {
    let events = vec![live_match(), {
        let mut other = event("foot:2", "Brest", "Nantes", State::In, NOW - 1800.0);
        other.abbreviations = vec!["SB29".into(), "FCN".into()];
        other
    }];

    let both = crate::sport::read_event(&reading("Lens", "Lille", "direct", 0.9), &events)
        .map(|(e, c)| (e.id.clone(), c));
    let one =
        crate::sport::read_event(&reading("Brest", "?", "direct", 0.9), &events).map(|(_, c)| c);
    let summary = crate::sport::read_event(&reading("Lens", "Lille", "resume", 0.9), &events);
    let unsure = crate::sport::read_event(&reading("Lens", "Lille", "direct", 0.2), &events);
    let unknown = crate::sport::read_event(&reading("Metz", "Reims", "direct", 0.9), &events);
    let ended = [event("foot:1", "Lens", "Lille", State::Post, NOW)];
    let finished = crate::sport::read_event(&reading("Lens", "Lille", "direct", 0.9), &ended);

    assert_eq!(both, Some(("foot:1".to_string(), Certainty::Seen)));
    assert_eq!(one, Some(Certainty::ToConfirm));
    assert!(summary.is_none() && unsure.is_none() && unknown.is_none() && finished.is_none());
}
