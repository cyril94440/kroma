use serde_json::{json, Value};

use super::{failure, int_arg, method_missing, room_reason, room_refusal, text_arg, Availability, Kind, Spec, ToolEnv};
use crate::paris::clock_label;
use crate::webcache::normalise;

const MAX_RAW: usize = 300;

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "party_go_live",
            kind: Kind::Refusable,
            activity: None,
            available: |a: &Availability| a.on("goLive"),
            describe: || {
                (
                    "Ramène TOUTE la watch party au direct : chaque membre saute au bord du flux en gardant la synchro. À utiliser quand on te demande « remets la watch party au direct », « on est trop en retard, ramène tout le monde au live ». Le système annonce lui-même l'action ; toi, reply UNE phrase courte. Refusé s'il n'y a pas de watch party active (dis-le alors).",
                    json!({ "type": "object", "properties": {} }),
                )
            },
            handler: go_live,
        },
        Spec {
            name: "party_fine_sync",
            kind: Kind::Refusable,
            activity: None,
            available: |a: &Availability| a.on("fineSync"),
            describe: || {
                (
                    "Synchronisation FINE de la watch party, à la demande (« synchronise-nous au poil », « synchro parfaite », « affine la synchro »). Chaque membre tente un calage à la frame ; le mode normal reprend ensuite. Nécessite au moins 2 membres. Le système annonce le lancement. Relancer est sans risque et peut faire mieux : si on te demande d'améliorer encore, RELANCE au lieu de dire que c'est impossible. Toi, reply UNE phrase courte.",
                    json!({ "type": "object", "properties": {} }),
                )
            },
            handler: fine_sync,
        },
        Spec {
            name: "get_client_logs",
            kind: Kind::Investigation,
            activity: Some("status"),
            available: |a: &Availability| a.on("clientLogs"),
            describe: || {
                (
                    "Journal de bord des SPECTATEURS : ce que vivent les apps (iPhone, Apple TV) et le web CÔTÉ CLIENT — gels vidéo, relances du player, erreurs de lecture, coupures/reconnexions, changements de chaîne, passages en arrière-plan. Complémentaire de run_ops(diagnose) qui voit le côté SERVEUR. À utiliser quand quelqu'un dit « ça freeze », « ça a coupé chez moi », « que se passe-t-il ? », ou pour investiguer a posteriori (« que s'est-il passé hier soir ? » -> minutes=1440). Filtre pseudo=… si la plainte vise une personne. Puis reply FACTUEL en citant les données (qui, quelle plateforme, combien de gels, quelles erreurs).",
                    json!({ "type": "object", "properties": {
                        "minutes": { "type": "integer", "description": "Fenêtre en minutes (défaut 30, max 4320 = 72 h)" },
                        "pseudo": { "type": "string", "description": "Filtrer sur un spectateur (optionnel)" },
                        "categorie": { "type": "string", "description": "Filtrer : player, ws, app, erreur (optionnel)" },
                        "limit": { "type": "integer", "description": "Nb max d'événements bruts (défaut 120, max 300)" },
                    } }),
                )
            },
            handler: client_logs,
        },
    ]
}

fn party_action(env: &mut ToolEnv, method: &'static str, result: anyhow::Result<Value>) -> Value {
    env.deps.audit("decisions", method, json!({ "user": env.message.user_id, "result": format!("{result:?}") }));
    match result {
        Ok(value) => {
            let mut out = json!({ "ok": true, "note": "le système annonce l'action dans le chat : reply UNE phrase courte" });
            if let Some(members) = value.get("members") {
                out["membres"] = members.clone();
            }
            out
        }
        Err(e) if method_missing(env, method, &e) => failure("fonction indisponible : le module de la watch party ne la propose pas"),
        Err(e) => {
            room_refusal(format!("{e:#}").trim())
        }
    }
}

fn go_live(env: &mut ToolEnv, _args: &Value) -> Value {
    let user = env.message.user_id.clone();
    let result = env.deps.room.go_live(&user);
    party_action(env, "goLive", result)
}

fn fine_sync(env: &mut ToolEnv, _args: &Value) -> Value {
    let user = env.message.user_id.clone();
    let result = env.deps.room.fine_sync(&user);
    party_action(env, "fineSync", result)
}

fn text_of<'a>(entry: &'a Value, key: &str) -> &'a str {
    entry.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn summarise(events: &[Value]) -> Value {
    let mut people: Vec<(String, Vec<&Value>)> = Vec::new();
    for entry in events {
        let name = text_of(entry, "name").to_string();
        match people.iter_mut().find(|(n, _)| *n == name) {
            Some((_, list)) => list.push(entry),
            None => people.push((name, vec![entry])),
        }
    }
    people
        .into_iter()
        .map(|(name, list)| {
            let mut counts: Vec<(String, usize)> = Vec::new();
            for e in &list {
                let key = format!("{}/{}", text_of(e, "category"), text_of(e, "event"));
                match counts.iter_mut().find(|(k, _)| *k == key) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((key, 1)),
                }
            }
            counts.sort_by_key(|c| std::cmp::Reverse(c.1));
            json!({ "pseudo": name, "appareil": list.last().map(|e| text_of(e, "device")), "evenements": list.len(), "detail": counts.into_iter().take(8).map(|(k, n)| json!({ "evenement": k, "nombre": n })).collect::<Vec<_>>() })
        })
        .collect()
}

fn client_logs(env: &mut ToolEnv, args: &Value) -> Value {
    let minutes = int_arg(args, "minutes", 30).clamp(1, 4320) as u32;
    let limit = int_arg(args, "limit", 120).clamp(1, MAX_RAW as i64) as usize;
    let (pseudo, category) = (text_arg(args, "pseudo"), text_arg(args, "categorie").to_lowercase());
    env.deps.audit("sources", "get_client_logs", json!({ "minutes": minutes, "pseudo": pseudo, "category": category }));
    let author = env.message.user_id.clone();
    let all = match env.deps.room.client_logs(&author, None, MAX_RAW as u32, Some(minutes)) {
        Ok(events) => events,
        Err(e) if method_missing(env, "clientLogs", &e) => return failure("journal indisponible : le module de la watch party ne le propose pas"),
        Err(e) if room_reason(format!("{e:#}").trim()).is_some() => return room_refusal(format!("{e:#}").trim()),
        Err(e) => return failure(&format!("journal indisponible : {e:#}")),
    };
    let wanted = normalise(&pseudo);
    let kept: Vec<Value> = all
        .into_iter()
        .filter(|e| wanted.is_empty() || normalise(text_of(e, "name")) == wanted || normalise(text_of(e, "userId")) == wanted)
        .filter(|e| category.is_empty() || text_of(e, "category").to_lowercase() == category)
        .collect();
    let raw: Vec<Value> = kept.iter().rev().take(limit).map(|e| json!({ "a": e.get("at").and_then(Value::as_i64).map(clock_label), "pseudo": text_of(e, "name"), "appareil": text_of(e, "device"), "categorie": text_of(e, "category"), "evenement": text_of(e, "event"), "donnees": e.get("data") })).collect();
    json!({ "minutes": minutes, "total": kept.len(), "par_spectateur": summarise(&kept), "evenements": raw })
}
