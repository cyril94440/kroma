use serde_json::{json, Value};

use super::{always, failure, int_arg, room_reason, room_refusal, text_arg, Kind, Spec, ToolEnv};
use crate::peers::Viewer;
use crate::webcache::normalise;

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "start_poll",
            kind: Kind::Refusable,
            activity: None,
            available: always,
            describe: || {
                (
                    "Lance un sondage chez tous les spectateurs (le code gouverne votes, clôture et résultats). 'choice' SEULEMENT si les options sont évidentes et exhaustives ; 'open' dès que l'espace des réponses est inconnu ; 'score' pour un pronostic de score ; 'yesno' pour une question fermée. N'invente jamais d'options. Après un succès, une phrase d'ambiance suffit : le système a déjà annoncé la question.",
                    json!({ "type": "object", "properties": {
                        "question": { "type": "string" },
                        "type": { "type": "string", "enum": ["yesno", "choice", "score", "open"] },
                        "options": { "type": "array", "items": { "type": "string" }, "description": "choice : 2 à 6 options" },
                        "duration_s": { "type": "integer", "description": "défaut 120, max 900" },
                    }, "required": ["question", "type"] }),
                )
            },
            handler: start_poll,
        },
        Spec {
            name: "close_poll",
            kind: Kind::Refusable,
            activity: None,
            available: always,
            describe: || ("Clôt le sondage en cours et renvoie les résultats détaillés (qui a voté quoi).", json!({ "type": "object", "properties": {} })),
            handler: close_poll,
        },
        Spec {
            name: "record_poll_vote",
            kind: Kind::Refusable,
            activity: None,
            available: always,
            describe: || {
                (
                    "Enregistre le vote de l'AUTEUR du message, exprimé en langage naturel pendant un sondage (« je vote pour l'Espagne », « 2-1 pour moi »). Ne confirme le vote QUE si l'outil renvoie ok:true.",
                    json!({ "type": "object", "properties": { "value": { "type": "string", "description": "valeur extraite du message" } }, "required": ["value"] }),
                )
            },
            handler: record_poll_vote,
        },
        Spec {
            name: "kick_from_party",
            kind: Kind::Refusable,
            activity: None,
            available: always,
            describe: || {
                (
                    "Retire quelqu'un de la watch party (bénin et réversible : il peut re-rejoindre en un clic). À appeler directement quand quelqu'un le demande ; le serveur décide seul (personne dans la watch party et connectée, pas retirée depuis moins de 2 minutes) et peut refuser.",
                    json!({ "type": "object", "properties": {
                        "pseudo": { "type": "string", "description": "nom affiché de la personne" },
                        "reason": { "type": "string" },
                    }, "required": ["pseudo"] }),
                )
            },
            handler: kick_from_party,
        },
    ]
}

fn start_poll(env: &mut ToolEnv, args: &Value) -> Value {
    let question = text_arg(args, "question");
    let kind = text_arg(args, "type");
    let options: Vec<String> = args.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let duration = int_arg(args, "duration_s", 120).clamp(10, 900);
    let result = env.deps.room.start_poll(&env.message.user_id, &question, &kind, &options, Some(duration));
    env.deps.audit("decisions", "start_poll", json!({ "user": env.message.user_id, "question": question, "type": kind, "options": options, "result": format!("{result:?}") }));
    match result {
        Ok(poll) => json!({ "ok": true, "poll": poll }),
        Err(e) if room_reason(format!("{e:#}").trim()).is_some() => room_refusal(format!("{e:#}").trim()),
        Err(e) => failure(&format!("lancement impossible : {e:#}")),
    }
}

fn close_poll(env: &mut ToolEnv, _args: &Value) -> Value {
    let result = env.deps.room.close_poll(&env.message.user_id);
    env.deps.audit("decisions", "close_poll", json!({ "user": env.message.user_id, "result": format!("{result:?}") }));
    match result {
        Ok(results) => json!({ "ok": true, "resultats": results }),
        Err(e) if room_reason(format!("{e:#}").trim()).is_some() => room_refusal(format!("{e:#}").trim()),
        Err(e) => failure(&format!("clôture impossible : {e:#}")),
    }
}

fn record_poll_vote(env: &mut ToolEnv, args: &Value) -> Value {
    let value = text_arg(args, "value");
    let result = env.deps.room.vote_poll(&env.message.user_id, &value);
    env.deps.audit("decisions", "record_poll_vote", json!({ "user": env.message.user_id, "value": value, "result": format!("{result:?}") }));
    match result {
        Ok(()) => json!({ "ok": true }),
        Err(e) => failure(&format!("vote impossible : {e:#}")),
    }
}

fn find_viewer(viewers: &[Viewer], name: &str) -> Result<Viewer, String> {
    let wanted = normalise(name);
    if wanted.is_empty() {
        return Err("pseudo est obligatoire".into());
    }
    if let Some(v) = viewers.iter().find(|v| normalise(&v.name) == wanted) {
        return Ok(v.clone());
    }
    let near: Vec<&Viewer> = viewers.iter().filter(|v| normalise(&v.name).contains(&wanted)).collect();
    match near.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(format!("personne ne s'appelle « {name} » parmi les spectateurs présents : {}", viewers.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(", "))),
        many => Err(format!("plusieurs spectateurs correspondent à « {name} » : {}", many.iter().map(|v| v.name.clone()).collect::<Vec<_>>().join(", "))),
    }
}

fn kick_from_party(env: &mut ToolEnv, args: &Value) -> Value {
    let name = text_arg(args, "pseudo");
    let reason = text_arg(args, "reason");
    let viewers = match env.deps.control.status() {
        Ok(status) => status.viewers,
        Err(e) => return failure(&format!("présence illisible : {e:#}")),
    };
    let target = match find_viewer(&viewers, &name) {
        Ok(v) => v,
        Err(why) => {
            env.deps.audit("decisions", "kick_refused", json!({ "requested_by": env.message.user_id, "name": name, "why": why }));
            return failure(&why);
        }
    };
    let result = env.deps.room.kick(&target.user_id, &env.message.user_id, &reason);
    env.deps.audit("decisions", "kick", json!({ "requested_by": env.message.user_id, "target": target.user_id, "reason": reason, "result": format!("{result:?}") }));
    match result {
        Ok(out) if out.outcome == "kicked" => json!({ "ok": true, "personne": target.name, "perturbations_recentes": out.disruptions, "note": "il peut re-rejoindre quand il veut" }),
        Ok(out) => room_refusal(out.reason.as_deref().unwrap_or("refusé")),
        Err(e) => failure(&format!("appel impossible : {e:#}")),
    }
}
