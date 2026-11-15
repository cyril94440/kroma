use super::competitions::{competitions_cited, is_special, special_cited};
use super::flat::FlatMatch;
use super::image::read_image;
use crate::sport::Reading;
use crate::text::{contains_words, norm};

pub const NEXT_PROGRAMME_MAX_S: f64 = 35.0 * 60.0;
const NEXT_PROGRAMME_WEIGHT: f64 = 0.7;
const COMMON_STADIUM_WORDS: [&str; 9] = [
    "stadium", "stadion", "stadio", "estadio", "stade", "arena", "complex", "stadyum", "ground",
];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DetectProgramme {
    pub title: String,
    pub hay: String,
    pub start: Option<f64>,
    pub stop: Option<f64>,
}

type Category = (&'static str, f64, String);

fn upsert(
    categories: &mut Vec<Category>,
    key: &'static str,
    points: f64,
    reason: impl Into<String>,
) {
    categories.push((key, points, reason.into()));
}

fn side_in(text: &str, found: &FlatMatch, home: bool) -> bool {
    found
        .forms(home, false)
        .iter()
        .any(|f| contains_words(text, f))
}

fn score_programme(
    found: &FlatMatch,
    programme: &DetectProgramme,
    following: bool,
) -> Vec<Category> {
    let title = norm(&programme.title);
    let text = format!("{title} {}", norm(&programme.hay))
        .trim()
        .to_string();
    let mut categories = Vec::new();
    if text.is_empty() {
        return categories;
    }
    let in_title: Vec<bool> = [true, false]
        .iter()
        .map(|h| side_in(&title, found, *h))
        .collect();
    let in_text: Vec<bool> = [true, false]
        .iter()
        .map(|h| side_in(&text, found, *h))
        .collect();
    if in_title.iter().all(|t| *t) {
        upsert(
            &mut categories,
            "equipes",
            10.0,
            "les deux équipes dans le titre du guide",
        );
    } else {
        let (mut points, mut reasons) = (0.0, Vec::new());
        if in_title.iter().any(|t| *t) {
            points += 3.0;
            reasons.push("une équipe dans le titre du guide".to_string());
        }
        let rest = (0..2).filter(|i| in_text[*i] && !in_title[*i]).count();
        if rest > 0 {
            points += 3.0 * rest as f64;
            reasons.push(format!("{rest} équipe(s) dans la description du guide"));
        }
        if points > 0.0 {
            upsert(&mut categories, "equipes", points, reasons.join(" + "));
        }
    }
    let cited = competitions_cited(&text);
    if let (false, Some(slug)) = (cited.is_empty(), found.slug()) {
        if cited.contains(slug) {
            upsert(
                &mut categories,
                "competition",
                2.0,
                "compétition citée par le guide",
            );
        } else if !(cited.contains("club.friendly") || cited.contains("fifa.friendly")) {
            upsert(
                &mut categories,
                "competition",
                -3.0,
                "le guide cite une autre compétition",
            );
        }
    }
    let venue = norm(&found.venue);
    if !venue.is_empty() {
        if contains_words(&text, &venue) {
            upsert(&mut categories, "stade", 5.0, "stade cité par le guide");
        } else if venue
            .split(' ')
            .any(|m| m.len() >= 6 && !COMMON_STADIUM_WORDS.contains(&m) && contains_words(&text, m))
        {
            upsert(
                &mut categories,
                "stade",
                3.0,
                "nom du stade reconnu dans le guide",
            );
        }
    }
    let city = norm(&found.city);
    let city_is_a_team = [true, false]
        .iter()
        .any(|h| found.forms(*h, false).contains(&city));
    if city.len() >= 4 && contains_words(&text, &city) && !city_is_a_team {
        upsert(&mut categories, "ville", 1.0, "ville citée par le guide");
    }
    if let (true, Some(start), Some(kickoff)) = (
        categories.iter().any(|c| c.1 > 0.0),
        programme.start,
        found.start,
    ) {
        let gap = (start - kickoff).abs();
        if gap <= 600.0 {
            upsert(
                &mut categories,
                "ecart",
                2.0,
                "début du programme proche du coup d'envoi",
            );
        } else if gap <= 1500.0 {
            upsert(
                &mut categories,
                "ecart",
                1.0,
                "début du programme assez proche du coup d'envoi",
            );
        }
    }
    if following {
        categories.iter_mut().for_each(|c| {
            c.1 *= NEXT_PROGRAMME_WEIGHT;
            c.2.push_str(" (programme suivant)");
        });
    }
    categories
}

pub fn useful_programmes(
    programmes: &[DetectProgramme],
    now: f64,
) -> Vec<(&DetectProgramme, bool)> {
    programmes
        .iter()
        .filter_map(|p| {
            let start = p.start?;
            if start <= now && p.stop.is_none_or(|stop| now < stop) {
                Some((p, false))
            } else if now < start && start <= now + NEXT_PROGRAMME_MAX_S {
                Some((p, true))
            } else {
                None
            }
        })
        .collect()
}

pub fn rate(
    found: &FlatMatch,
    programmes: &[DetectProgramme],
    now: f64,
    image: Option<&Reading>,
    candidates: &[FlatMatch],
) -> (f64, Vec<String>) {
    let mut best: Vec<Category> = Vec::new();
    let mut guide_text = String::new();
    for (programme, following) in useful_programmes(programmes, now) {
        guide_text.push_str(&format!(
            " {} {}",
            norm(&programme.title),
            norm(&programme.hay)
        ));
        for (key, points, reason) in score_programme(found, programme, following) {
            match best.iter_mut().find(|c| c.0 == key) {
                Some(existing) if points > existing.1 => *existing = (key, points, reason),
                Some(_) => {}
                None => best.push((key, points, reason)),
            }
        }
    }
    if !best.iter().any(|c| c.0 != "ecart" && c.1 > 0.0) {
        best.retain(|c| c.0 != "ecart");
    }
    let mut total: f64 = best.iter().map(|c| c.1).sum();
    let mut reasons: Vec<String> = best
        .iter()
        .filter(|c| c.1 != 0.0)
        .map(|c| c.2.clone())
        .collect();
    let (image_points, image_reasons) = read_image(found, image, candidates);
    total += image_points;
    reasons.extend(image_reasons);
    if is_special(&found.competition, &found.home, &found.away) && !special_cited(&guide_text) {
        total -= 20.0;
        reasons.push("féminine / jeunes / réserve non annoncée par le guide".into());
    }
    (total, reasons)
}
