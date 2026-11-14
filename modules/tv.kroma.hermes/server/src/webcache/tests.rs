use super::*;

fn entry(rank: f64, title: &str, channel: &str) -> Entry {
    Entry {
        rank,
        title: title.into(),
        matched: 2,
        channel_id: channel.into(),
        channel_name: channel.into(),
        start: 0,
        end: 0,
        domestic: true,
    }
}

#[test]
fn the_cache_expires_and_keys_on_the_normalised_request() {
    let cache = TtlCache::new(600_000);
    cache.put("Le Match de l'Argentine ?", vec!["Argentine".to_string()], 0);
    assert_eq!(cache.get("le match de l argentine", 1000), Some(vec!["Argentine".to_string()]));
    assert_eq!(cache.get("le match de l'argentine", 601_000), None);
}

#[test]
fn the_cache_is_bounded() {
    let cache = TtlCache::with_capacity(600_000, 2);
    for i in 0..5 {
        cache.put(&format!("q{i}"), i, i);
    }
    assert_eq!(cache.len(), 2);
}

#[test]
fn dated_questions_are_recognised() {
    for q in [
        "Quel est le score du PSG ?",
        "à quelle heure joue Lens",
        "qui a marqué pour Lens",
        "quand joue le PSG ?",
        "quand jouent les Bleus ?",
        "c'est quand le match du PSG ?",
        "quand est-ce que joue le PSG ?",
        "le match de ce soir ?",
        "le PSG joue quand",
        "quel match ce soir",
        "qui a mis les buts pour Lens",
    ] {
        assert!(question_dated(q), "{q}");
    }
    for q in ["c'est quoi le hors-jeu", "quand a été fondé le PSG", "c'est quoi un match nul", "c'est quoi le hors-jeu ?", "quand a été fondé le PSG ?"] {
        assert!(!question_dated(q), "{q}");
    }
}

#[test]
fn date_marks_follow_the_source_rules() {
    assert!(has_date_mark("Coup d'envoi à 21h00", 2026));
    assert!(has_date_mark("Ce soir, Lens mène 1-0", 2026));
    assert!(has_date_mark("Coup d'envoi à 21 heures", 2026));
    assert!(has_date_mark("Victoire 2-1 du PSG en 2026.", 2026));
    assert!(!has_date_mark("Lens a battu Paris 2-1", 2026));
    assert!(!has_date_mark("Fondé en 1970", 2026));
    assert!(!has_date_mark("Victoire 2-1 du PSG en 2024.", 2026));
    assert!(!has_date_mark("Il mène de 1.5 buts", 2026));
}

#[test]
fn one_title_on_two_channels_is_one_programme() {
    assert!(ambiguous(&[entry(10.0, "Tour de France", "TF1"), entry(10.0, "Tour de France", "F3")], "", |e| e.rank).is_empty());
}

#[test]
fn two_distinct_titles_of_equal_rank_are_ambiguous() {
    let live = [entry(10.0, "Rugby : France / Australie", "TF1"), entry(10.0, "Football : France / Australie", "F3")];
    assert_eq!(ambiguous(&live, "", |e| e.rank).len(), 2);
}

#[test]
fn a_better_channel_is_not_ambiguous() {
    assert!(ambiguous(&[entry(10.0, "A b", "TF1"), entry(2.0, "C d", "F3")], "", |e| e.rank).is_empty());
}

#[test]
fn the_title_equal_to_the_request_wins() {
    let live = [entry(10.0, "Tour de France", "TF1"), entry(10.0, "Tour de France : le mag", "F3")];
    assert!(ambiguous(&live, "tour de france", |e| e.rank).is_empty());
    let mut late = vec![entry(10.0, "Tour de France : le mag", "F3"), entry(10.0, "Tour de France", "F2")];
    exact_title_first(&mut late, "tour de france");
    assert_eq!(late[0].title, "Tour de France");
    assert!(ambiguous(&late, "tour de france", |e| e.rank).is_empty());
}

#[test]
fn foreign_channels_sort_last_then_exact_title_then_rank() {
    let mut foreign = entry(1.0, "Tour de France", "BE");
    foreign.domestic = false;
    let mut live = vec![foreign, entry(6.4, "Cyclisme : Tour de France", "FR"), entry(7.0, "Tour de France", "VIP")];
    exact_title_first(&mut live, "tour de france");
    assert_eq!(live.iter().map(|e| e.channel_name.as_str()).collect::<Vec<_>>(), vec!["VIP", "FR", "BE"]);
}

#[test]
fn a_quality_bonus_does_not_hide_an_ambiguity_at_country_class() {
    let live = [entry(10.6, "Rugby : A / B", "TF1"), entry(10.4, "Foot : A / B", "F3")];
    assert_eq!(ambiguous(&live, "a b", |_| 6.0).len(), 2);
    assert!(ambiguous(&live, "a b", |e| e.rank).is_empty());
}

#[test]
fn a_country_prefix_other_than_france_marks_a_foreign_channel() {
    assert!(is_foreign("UK: BBC One"));
    assert!(is_foreign("AL | RTSH"));
    assert!(!is_foreign("FR: TF1"));
    assert!(!is_foreign("TF1"));
    assert!(!is_foreign("Canal+ VIP RAW"));
}
