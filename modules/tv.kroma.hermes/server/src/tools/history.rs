use serde_json::{json, Value};

use super::chart::{build, Measure, STEP_S};
use super::{int_arg, method_missing, text_arg, Availability, Kind, Spec, ToolEnv};

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "stream_history",
        kind: Kind::Investigation,
        activity: Some("history"),
        available: |a: &Availability| a.on("history"),
        describe: || {
            (
                "HISTORIQUE du flux (tenu par le module live, un point toutes les 10 s, 3 h au plus) : débit reçu du fournisseur ou saccades. Rend un RÉSUMÉ chiffré (moyenne, minimum, épisodes de panne ou de gel, chaînes vues, redémarrages) et une CARTE chart déjà prête : pour la montrer, appelle ensuite show_card avec cette carte TELLE QUELLE (ne recalcule rien, ne change aucun chiffre). Ex: « trace le débit de la dernière heure », « ça saccade depuis quand ? ». Lecture seule, aucune connexion au fournisseur.",
                json!({ "type": "object", "properties": {
                    "minutes": { "type": "integer", "description": "Fenêtre en minutes (défaut 60, max 180)" },
                    "mesure": { "type": "string", "enum": ["debit", "saccades"], "description": "debit = kb/s reçus (défaut) ; saccades = secondes perdues par minute" },
                } }),
            )
        },
        handler: stream_history,
    }]
}

fn stream_history(env: &mut ToolEnv, args: &Value) -> Value {
    let minutes = int_arg(args, "minutes", 60).clamp(1, 180);
    let Some(measure) = Measure::parse(&text_arg(args, "mesure")) else { return json!({ "erreur": "mesure inconnue : debit ou saccades" }) };
    let limit = (minutes * 60 / STEP_S) as u32;
    let history = match env.deps.control.history(limit) {
        Ok(history) => history,
        Err(e) => {
            method_missing(env, "history", &e);
            return json!({ "erreur": format!("historique indisponible : {e:#}") });
        }
    };
    env.deps.audit("sources", "stream_history", json!({ "minutes": minutes, "measure": text_arg(args, "mesure"), "points": history.points.len() }));
    let latest = history.points.last().map_or(0, |p| p.at);
    let window: Vec<_> = history.points.into_iter().filter(|p| latest - p.at < minutes * 60).collect();
    let mut out = build(&window, minutes, measure);
    if out.get("carte").is_some() && !history.provider_events.is_empty() {
        out["bascules_fournisseur"] = Value::Array(history.provider_events.iter().rev().take(5).cloned().collect());
    }
    out
}
