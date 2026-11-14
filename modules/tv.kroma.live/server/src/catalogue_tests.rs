use crate::catalogue::Catalogue;
use crate::model::{Channel, ProviderId};
use crate::norm::{chan_number, channel_key, family_slot, fold, is_separator};
use crate::rules::Rules;

fn fake(provider: ProviderId, names: &[(&str, Option<&str>)]) -> Catalogue {
    Catalogue::new(
        names
            .iter()
            .enumerate()
            .map(|(at, (name, epg))| Channel::new(provider, at as i64, name, *epg, ""))
            .collect(),
    )
}

fn primary(names: &[(&str, Option<&str>)]) -> Catalogue {
    fake(ProviderId::Primary, names)
}

fn same(a: &str, b: &str) -> bool {
    channel_key(&fold(a)) == channel_key(&fold(b))
}

fn names(channels: Vec<&Channel>) -> Vec<String> {
    channels.into_iter().map(|c| c.name.clone()).collect()
}

fn multiplexes() -> (Catalogue, Catalogue) {
    let mut prim: Vec<(String, Option<&str>)> = vec![
        ("FR: LIGUE 1+ ◉ ᴿᴬᵂ".into(), None),
        ("FR: BEIN SPORTS 1 FHD".into(), Some("beinsports1.fr")),
        ("FR: BEIN SPORTS 2 FHD".into(), Some("beinsports2.fr")),
        ("FR: BEIN SPORTS 3 FHD".into(), Some("beinsports3.fr")),
        ("FR: TF1 FHD".into(), Some("tf1.fr")),
        ("VIP: CANAL+ ᴿᴬᵂ".into(), Some("canalplus.fr")),
    ];
    for n in 1..=10 {
        for pattern in ["VIP: LIGUE 1+ {n} ᴿᴬᵂ", "VIP: LIGUE 1+ {n} ᴴᴰ", "FR: LIGUE1+ ᴮᴱ {n} ᴿᴬᵂ", "FR: LIGUE1+ {n} ⱽᴵᴾ ᴴᴰ", "FR: LIGUE1+ {n} ⱽᴵᴾ ᴿᴬᵂ", "FR: LIGUE 1+ {n} ᴿᴬᵂ"] {
            prim.push((pattern.replace("{n}", &n.to_string()), None));
        }
    }
    for n in 1..=5 {
        prim.push((format!("UK: LIGUE 1 PPV | MATCH {n}"), None));
    }
    let mut sec: Vec<(String, Option<&str>)> = ["FHD", "HD", "SD"].iter().map(|q| (format!("|FR| LIGUE1+ {q}"), None)).collect();
    for n in 2..=10 {
        for q in ["FHD", "HD", "SD"] {
            sec.push((format!("|FR| LIGUE1+ {n} {q}"), None));
        }
    }
    sec.push(("DAZN LIGUE 1 1 - Lens - Paris FC - 21:00".into(), None));
    sec.push(("DAZN LIGUE 1 2 - Lyon - Monaco - 17:00".into(), None));
    for n in 1..=3 {
        sec.push((format!("ViaPlay LIGUE1 FR {n} UHD"), Some("FR LIGUE1 1")));
    }
    let lift = |list: Vec<(String, Option<&str>)>, provider| {
        Catalogue::new(list.iter().enumerate().map(|(at, (n, e))| Channel::new(provider, at as i64, n, *e, "")).collect())
    };
    (lift(prim, ProviderId::Primary), lift(sec, ProviderId::Backup))
}

#[test]
fn two_names_for_one_channel_share_a_key_and_a_variant_marker_is_not_a_country() {
    assert!(same("VIP: LIGUE 1+ 3 ᴿᴬᵂ", "|FR| LIGUE1+ 3 FHD"));
    assert!(same("FR: LIGUE1+ ᴮᴱ 5 ᴿᴬᵂ", "|FR| LIGUE1+ 5 FHD"));
    assert!(!same("FR: LIGUE1+ 3 ⱽᴵᴾ ᴿᴬᵂ", "|FR| LIGUE1+ 5 FHD"));
    assert!(same("VIP: CANAL+ ᴿᴬᵂ", "FR: CANAL+ FHD"));
    assert!(same("VIP: RMC SPORT 1 ᴿᴬᵂ", "FR: FRA | RMC Sport 1 FHD"));
    assert!(same("FR: TMC +1 HEVC", "FR: TMC+1 FHD") && !same("FR: TMC+1 FHD", "FR: TMC FHD"));
    assert!(!same("SG: HUB beIN SPORTS 1 ᴿᴬᵂ", "FR: BEIN SPORTS 1 FHD"));
    assert!(is_separator("•●★▼ |FR| LIGUE1+ |FR| ▼★●•") && is_separator("##### LIGUE1+ ᴮᴱ #####"));
    assert!(!is_separator("FR: CANAL+ FHD"));
}

#[test]
fn a_suggestion_ignores_short_words_and_ranks_by_common_words() {
    let catalogue = primary(&[
        ("PRIME: 13EME RUE ᴿᴬᵂ", None),
        ("PRIME: AB1 ᴿᴬᵂ", None),
        ("VIP: TF1 ᴿᴬᵂ", None),
        ("VIP: TV MONACO", None),
        ("FR: LIGUE1+ 1 ⱽᴵᴾ ᴿᴬᵂ", None),
        ("SR: ARENA SPORT 7", None),
        ("FR: PARIS PREMIERE", None),
    ]);
    let rules = Rules::default();

    let found = names(catalogue.suggest("Paris Monaco ligue 1", 8, &rules));

    assert!(!found.iter().any(|n| ["PRIME: 13EME RUE ᴿᴬᵂ", "PRIME: AB1 ᴿᴬᵂ", "VIP: TF1 ᴿᴬᵂ"].contains(&n.as_str())), "{found:?}");
    assert!(["FR: LIGUE1+ 1 ⱽᴵᴾ ᴿᴬᵂ", "FR: PARIS PREMIERE"].contains(&found[0].as_str()), "{found:?}");
    assert!(found.contains(&"VIP: TV MONACO".to_string()));
    assert_eq!(names(catalogue.suggest("tf1", 8, &rules)), ["VIP: TF1 ᴿᴬᵂ"]);
}

#[test]
fn an_equivalent_at_the_other_provider_is_found_by_name_then_through_the_guide() {
    let prim = primary(&[
        ("VIP: L'EQUIPE 21 ᴴᴰ", Some("lequipe21.fr")),
        ("FR: L'EQUIPE 21 ᴿᴬᵂ", None),
        ("FR: RMC SPORT 2 ᴿᴬᵂ", Some("rmcsport2.fr")),
        ("FR: LIGUE1+ 1 ⱽᴵᴾ ᴿᴬᵂ", None),
    ]);
    let sec = fake(ProviderId::Backup, &[
        ("FR: L EQUIPE FHD", Some("LEquipe21.fr")),
        ("FR: L EQUIPE SD", None),
        ("FR: FRA | RMC Sport 5 FHD", Some("rmcsport2.fr")),
        ("FR: CANAL+ UHD", None),
        ("|FR| LIGUE1+ FHD", None),
        ("|FR| LIGUE1+ 2 FHD", None),
    ]);
    let rules = Rules::default();
    let first = |name: &str| sec.equivalents(name, Some(&prim), 1, &rules).first().map(|c| c.name.clone());

    assert_eq!(first("FR: L'EQUIPE 21 ᴿᴬᵂ").as_deref(), Some("FR: L EQUIPE FHD"));
    assert_eq!(names(sec.equivalents("FR: L'EQUIPE 21 ᴿᴬᵂ", Some(&prim), 3, &rules)), ["FR: L EQUIPE FHD", "FR: L EQUIPE SD"]);
    assert_eq!(first("FR: RMC SPORT 2 ᴿᴬᵂ"), None);
    assert_eq!(first("FR: LIGUE1+ 1 ⱽᴵᴾ ᴿᴬᵂ").as_deref(), Some("|FR| LIGUE1+ FHD"));
    assert_eq!(first("VIP: CANAL+ ᴿᴬᵂ"), None);
}

#[test]
fn a_8k_channel_is_found_by_its_name_and_never_without_asking() {
    let catalogue = primary(&[("FR: TF1 FHD", None), ("FR: ARTE 8K", None), ("FR: ARTE FHD", None)]);
    let rules = Rules::default();

    assert_eq!(catalogue.find("arte 8k", &rules).map(|c| c.name.as_str()), Some("FR: ARTE 8K"));
    assert_eq!(catalogue.find("arte", &rules).map(|c| c.name.as_str()), Some("FR: ARTE FHD"));
}

#[test]
fn allowing_4k_in_the_rules_lets_the_automatic_choice_take_it() {
    let catalogue = primary(&[("FR: ARTE 4K", None)]);
    let open = Rules { allow_4k: true, ..Rules::default() };

    assert!(catalogue.find("arte", &Rules::default()).is_none());
    assert!(catalogue.find("arte", &open).is_some());
}

#[test]
fn a_multiplex_number_is_neither_the_one_of_ligue_1_nor_a_time() {
    let number = |name: &str| chan_number(&fold(name));
    assert_eq!(number("DAZN LIGUE 1 4 - Lyon - Monaco - 17:00"), 4);
    assert_eq!(number("UK: LIGUE 1 PPV | MATCH 3"), 3);
    assert_eq!((number("|FR| LIGUE1+ 10 FHD"), number("VIP: LIGUE 1+ 3 ᴿᴬᵂ")), (10, 3));
    assert_eq!((number("|FR| LIGUE1+ FHD"), number("FR: LIGUE 1+ ◉ ᴿᴬᵂ")), (999, 999));
    assert_eq!(number("FR: BEIN SPORTS 2 FHD"), 2);
}

#[test]
fn a_family_and_its_slot_ignore_prefixes_quality_and_the_match_poster() {
    let slot = |name: &str| {
        let (family, number) = family_slot(&fold(name));
        (family.region, family.head, number)
    };
    let expect = |region: &str, head: &str, number: Option<u32>| (region.to_string(), head.to_string(), number);
    assert_eq!(slot("VIP: LIGUE 1+ 3 ᴿᴬᵂ"), expect("fr", "ligue1+", Some(3)));
    assert_eq!(slot("|FR| LIGUE1+ 7 FHD"), expect("fr", "ligue1+", Some(7)));
    assert_eq!(slot("FR: LIGUE1+ ᴮᴱ 5 ᴿᴬᵂ"), expect("fr", "ligue1+", Some(5)));
    assert_eq!(slot("DAZN LIGUE 1 3 - Lens - Paris FC - 21:00"), expect("fr", "dazn ligue1", Some(3)));
    assert_eq!(slot("UK: LIGUE 1 PPV | MATCH 3"), expect("uk", "ligue1 ppv match", Some(3)));
    assert_eq!(slot("ViaPlay LIGUE1 FR 3 UHD"), expect("fr", "viaplay ligue1", Some(3)));
    assert_eq!(slot("|FR| LIGUE1+ FHD"), expect("fr", "ligue1+", None));
    assert_eq!(slot("EUROSPORT 360 1").1, "eurosport 360");
}

#[test]
fn an_event_multiplex_is_told_from_numbered_channels_that_have_a_guide() {
    let (prim, sec) = multiplexes();

    assert!(prim.is_event_channel("FR: LIGUE1+ 4 ⱽᴵᴾ ᴴᴰ"));
    assert!(prim.is_event_channel("UK: LIGUE 1 PPV | MATCH 3"));
    assert!(sec.is_event_channel("|FR| LIGUE1+ 4 FHD"));
    assert!(!prim.is_event_channel("FR: BEIN SPORTS 2 FHD"));
    assert!(!prim.is_event_channel("FR: TF1 FHD"));
}

#[test]
fn a_scan_gets_one_candidate_per_number_best_variant_first_and_the_asked_number_first() {
    let (prim, sec) = multiplexes();
    let rules = Rules::default();

    let candidates = sec.slot_candidates("ligue1+", 12, Some(4), &rules);

    assert_eq!(candidates.iter().map(|c| c.number).collect::<Vec<_>>(), [Some(4), Some(1), Some(2), Some(3), Some(5), Some(6), Some(7), Some(8), Some(9), Some(10)]);
    assert_eq!(candidates[0].channel.name, "|FR| LIGUE1+ 4 FHD");
    assert_eq!(candidates[0].alternatives.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["|FR| LIGUE1+ 4 HD", "|FR| LIGUE1+ 4 SD"]);
    assert_eq!(candidates[1].channel.name, "|FR| LIGUE1+ FHD");
    let own = prim.slot_candidates("ligue1+", 12, None, &rules);
    assert_eq!(own.iter().map(|c| c.number).collect::<Vec<_>>(), (1..=10).map(Some).chain([None]).collect::<Vec<_>>());
    assert_eq!(own[2].channel.name, "FR: LIGUE1+ 3 ⱽᴵᴾ ᴿᴬᵂ");
    assert_eq!(own[2].alternatives.len(), 5);
    let wide = sec.slot_candidates("ligue 1", 12, None, &rules);
    assert!(wide.iter().take(10).all(|c| c.family.head == "ligue1+") && wide.iter().skip(10).all(|c| c.family.head == "dazn ligue1"));
    assert!(!wide.iter().any(|c| c.channel.name.contains("UHD")));
}

#[test]
fn names_resolve_to_the_preferred_variant() {
    let (prim, _) = multiplexes();
    let rules = Rules::default();

    assert_eq!(prim.find("ligue1+ 3", &rules).map(|c| c.name.as_str()), Some("FR: LIGUE1+ 3 ⱽᴵᴾ ᴿᴬᵂ"));
    assert_eq!(prim.find("canal+ vip raw", &rules).map(|c| c.name.as_str()), Some("VIP: CANAL+ ᴿᴬᵂ"));
    assert_eq!(prim.find("l1+ 3", &rules).map(|c| c.name.as_str()), Some("FR: LIGUE1+ 3 ⱽᴵᴾ ᴿᴬᵂ"));
}

#[test]
fn a_french_channel_beats_a_foreign_one_and_raw_beats_hd() {
    let catalogue = primary(&[("BE: TF1 HD", None), ("FR: TF1 HD", None), ("VIP: TF1 ᴿᴬᵂ", None), ("SR: ARENA SPORT 7", None)]);
    let rules = Rules::default();

    let ranked = catalogue.find_all("tf1", 5, false, &rules);

    assert_eq!(ranked.iter().map(|(c, _)| c.name.as_str()).collect::<Vec<_>>(), ["VIP: TF1 ᴿᴬᵂ", "FR: TF1 HD", "BE: TF1 HD"]);
    assert!(catalogue.country_tier("SR: ARENA SPORT 7", &rules) < 0.0);
}

#[test]
fn a_channel_without_the_asked_quality_is_still_found_by_its_bare_name() {
    let catalogue = primary(&[("FR: CANAL+ FHD", None)]);

    assert_eq!(catalogue.find("canal+ vip raw", &Rules::default()).map(|c| c.name.as_str()), Some("FR: CANAL+ FHD"));
}

#[test]
fn the_best_channel_of_a_guide_id_never_takes_the_4k() {
    let catalogue = primary(&[("FR: TF1 4K", Some("TF1.fr")), ("FR: TF1 HD", Some("tf1.fr")), ("VIP: TF1 ᴿᴬᵂ", Some("tf1.fr"))]);

    assert_eq!(catalogue.best_for_epg("TF1.FR", &Rules::default()).map(|c| c.name.as_str()), Some("VIP: TF1 ᴿᴬᵂ"));
}

#[test]
fn references_name_a_provider_and_a_stream() {
    use crate::model::parse_ref;
    assert_eq!(parse_ref("s:4123"), Some((ProviderId::Backup, 4123)));
    assert_eq!(parse_ref("p:881"), Some((ProviderId::Primary, 881)));
    for bad in ["s:abc", "x:1", "http://h/1.ts", "TF1", ""] {
        assert_eq!(parse_ref(bad), None, "{bad}");
    }
}
