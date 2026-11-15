use serde_json::{json, Value};

use crate::paris::clock_label;
use crate::peers::HistoryPoint;

pub const STEP_S: i64 = 10;
const MAX_CARD_POINTS: usize = 24;
const MIN_POINTS: usize = 6;
const MAX_EPISODES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Rate,
    Stutter,
}

impl Measure {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "" | "debit" => Some(Measure::Rate),
            "saccades" => Some(Measure::Stutter),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Measure::Rate => "debit",
            Measure::Stutter => "saccades",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Measure::Rate => "kb/s",
            Measure::Stutter => "s perdue/min",
        }
    }

    fn value(self, point: &HistoryPoint) -> f64 {
        match self {
            Measure::Rate => point.kbps,
            Measure::Stutter => point.loss,
        }
    }

    fn round(self, value: f64) -> f64 {
        match self {
            Measure::Rate => value.round(),
            Measure::Stutter => (value * 10.0).round() / 10.0,
        }
    }
}

fn hour(point: &HistoryPoint) -> String {
    clock_label(point.at * 1000)
}

pub fn abnormal(point: &HistoryPoint) -> bool {
    !point.paused && (point.kbps <= 0.0 || point.frozen || point.black || matches!(point.health.as_deref(), Some("degraded" | "severe")))
}

pub fn episodes(points: &[HistoryPoint]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut run: Vec<&HistoryPoint> = Vec::new();
    for point in points.iter().map(Some).chain(std::iter::once(None)) {
        match point {
            Some(p) if abnormal(p) => run.push(p),
            _ => {
                if let (Some(first), Some(last)) = (run.first(), run.last()).filter(|_| run.len() >= 2) {
                    out.push(json!({ "debut": hour(first), "fin": hour(last), "duree_s": last.at - first.at + STEP_S }));
                }
                run.clear();
            }
        }
    }
    out
}

fn aggregate(points: &[HistoryPoint], measure: Measure) -> Vec<Value> {
    let n = points.len();
    let k = MAX_CARD_POINTS.min(n);
    (0..k)
        .map(|i| {
            let slice = &points[i * n / k..(i + 1) * n / k];
            let mean = slice.iter().map(|p| measure.value(p)).sum::<f64>() / slice.len() as f64;
            json!({ "label": hour(&slice[slice.len() / 2]), "value": measure.round(mean) })
        })
        .collect()
}

fn window_label(minutes: i64) -> String {
    if minutes % 60 == 0 { format!("{} h", minutes / 60) } else { format!("{minutes} min") }
}

pub fn build(points: &[HistoryPoint], minutes: i64, measure: Measure) -> Value {
    if points.len() < MIN_POINTS {
        return json!({ "erreur": "historique trop court pour tracer une courbe (le flux vient de démarrer ou la fenêtre est vide) : dis-le, ne l'invente pas", "points": points.len() });
    }
    let active: Vec<&HistoryPoint> = points.iter().filter(|p| !p.paused).collect();
    if active.is_empty() {
        return json!({ "erreur": "le flux était en veille (personne ne regardait) sur toute la période : rien à tracer, dis-le", "points": points.len() });
    }
    let values: Vec<f64> = active.iter().map(|p| measure.value(p)).collect();
    let mut worst = 0;
    for (i, value) in values.iter().enumerate() {
        let worse = if measure == Measure::Rate { *value < values[worst] } else { *value > values[worst] };
        if worse {
            worst = i;
        }
    }
    let eps = episodes(points);
    let mut seen: Vec<String> = Vec::new();
    for name in points.iter().filter_map(|p| p.channel.clone()) {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    let extreme = if measure == Measure::Rate { "minimum" } else { "maximum" };
    let mut summary = json!({
        "mesure": measure.name(), "fenetre_min": minutes, "unite": measure.unit(), "echantillons": points.len(),
        "moyenne": measure.round(values.iter().sum::<f64>() / values.len() as f64),
        "a": hour(active[worst]),
        "veille_s": (points.len() - active.len()) as i64 * STEP_S,
        "episodes": eps.iter().take(MAX_EPISODES).cloned().collect::<Vec<_>>(), "nb_episodes": eps.len(),
        "chaines": seen.into_iter().take(6).collect::<Vec<_>>(),
        "redemarrages": (points[points.len() - 1].restarts - points[0].restarts).max(0),
    });
    summary[extreme] = json!(measure.round(values[worst]));
    let title = format!("{} ({})", if measure == Measure::Rate { "Débit du flux" } else { "Saccades du flux" }, window_label(minutes));
    json!({
        "resume": summary,
        "carte": { "kind": "chart", "title": title, "style": "line", "unit": measure.unit(), "points": aggregate(points, measure) },
        "usage": "show_card(text=<phrase de repli avec les chiffres du résumé>, card=<carte>)",
    })
}
