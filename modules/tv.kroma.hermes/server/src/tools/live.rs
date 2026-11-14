use serde_json::{json, Value};

use super::{always, failure, int_arg, text_arg, Kind, Spec, ToolEnv};
use crate::paris::clock_label;
use crate::peers::{Candidate, Slot, Status};

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "change_channel",
            kind: Kind::Action,
            activity: Some("switch"),
            available: always,
            describe: || {
                (
                    "Change la chaîne POUR TOUT LE MONDE : le système applique lui-même la règle (bascule immédiate si le demandeur est seul, vote sinon) et l'annonce dans le chat. Seulement si quelqu'un demande explicitement de METTRE une chaîne, une seule fois par message. `ref` : identifiant renvoyé par list_channels ou find_program.",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string", "description": "Nom de la chaîne telle que demandée" },
                        "ref": { "type": "string", "description": "Identifiant de chaîne donné par un outil" },
                    } }),
                )
            },
            handler: change_channel,
        },
        Spec {
            name: "list_channels",
            kind: Kind::Investigation,
            activity: Some("channels"),
            available: always,
            describe: || {
                (
                    "Cherche dans le catalogue de chaînes (lecture seule, aucun effet sur la diffusion). Chaque chaîne porte un `ref` à passer à change_channel.",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string" },
                        "limit": { "type": "integer", "description": "défaut 20, max 80" },
                    }, "required": ["query"] }),
                )
            },
            handler: list_channels,
        },
        Spec {
            name: "get_status",
            kind: Kind::Investigation,
            activity: Some("status"),
            available: always,
            describe: || {
                (
                    "État du direct : chaîne en cours, programme, fournisseur, spectateurs présents et santé du flux.",
                    json!({ "type": "object", "properties": {} }),
                )
            },
            handler: get_status,
        },
        Spec {
            name: "run_ops",
            kind: Kind::Investigation,
            activity: Some("status"),
            available: always,
            describe: || {
                (
                    "Diagnostic en lecture seule. diagnose : verdict de synthèse sur un souci d'image ou de son (cite les chiffres) ; sante_flux : détail de la santé du flux ; fournisseurs : fournisseur actif.",
                    json!({ "type": "object", "properties": {
                        "action": { "type": "string", "enum": ["diagnose", "sante_flux", "fournisseurs"] },
                        "pseudo": { "type": "string", "description": "qui signale le souci (facultatif)" },
                    }, "required": ["action"] }),
                )
            },
            handler: run_ops,
        },
    ]
}

pub fn slot_json(slot: &Option<Slot>) -> Value {
    slot.as_ref().map_or(Value::Null, |s| json!({ "titre": s.title, "debut": clock_label(s.start), "fin": clock_label(s.end) }))
}

fn candidate_json(c: &Candidate) -> Value {
    json!({ "ref": c.id, "nom": c.name, "groupe": c.group, "en_cours": c.now.as_ref().map(|s| s.title.clone()) })
}

fn list_channels(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    let limit = int_arg(args, "limit", 20).clamp(1, 80) as u32;
    match env.deps.control.resolve(&query, limit) {
        Ok(found) => {
            env.deps.audit("sources", "list_channels", json!({ "query": query, "count": found.len(), "top": found.iter().take(5).map(|c| json!({ "id": c.id, "name": c.name, "score": c.score })).collect::<Vec<_>>() }));
            json!({ "chaines": found.iter().map(candidate_json).collect::<Vec<_>>() })
        }
        Err(e) => failure(&format!("catalogue indisponible : {e:#}")),
    }
}

pub fn status_json(status: &Status) -> Value {
    json!({
        "etat": status.state,
        "chaine": status.channel.as_ref().map(|c| json!({ "ref": c.id, "nom": c.name })),
        "fournisseur": status.provider,
        "en_ce_moment": slot_json(&status.now),
        "ensuite": slot_json(&status.next),
        "spectateurs": status.viewers.iter().map(|v| v.name.clone()).collect::<Vec<_>>(),
        "sante": {
            "debit_source_kbps": status.health.source_kbps.round(),
            "saccades": status.health.stutter,
            "image_noire": status.health.black,
            "image_figee": status.health.frozen,
        },
    })
}

fn get_status(env: &mut ToolEnv, _args: &Value) -> Value {
    match env.deps.control.status() {
        Ok(status) => status_json(&status),
        Err(e) => failure(&format!("état indisponible : {e:#}")),
    }
}

fn unstable(status: &Status) -> bool {
    status.health.stutter || status.health.black || status.health.frozen || status.state == "error" || status.state == "offAir"
}

fn verdict(status: &Status) -> &'static str {
    if status.state == "offAir" {
        "chaine_hors_antenne : la chaîne ne diffuse plus rien chez le fournisseur ; propose une alternative (list_channels) sans changer toi-même."
    } else if status.health.black || status.health.frozen {
        "image_noire_ou_figee : le flux arrive mais l'image est noire ou figée ; si c'est un multiplex, vérifie avec look_at_live puis cherche le programme ailleurs."
    } else if unstable(status) {
        "source_instable : le flux du fournisseur saccade. Tout le monde reçoit le même flux : ce n'est pas l'appareil de la personne. Conseille de patienter ou de changer de chaîne."
    } else if status.health.source_kbps > 0.0 {
        "serveur_stable : le flux est sain côté serveur. Les saccades viennent probablement de la connexion ou de l'appareil du spectateur (conseille de recharger la page, vérifier le wifi)."
    } else {
        "indetermine : pas de mesure de débit disponible pour l'instant."
    }
}

fn run_ops(env: &mut ToolEnv, args: &Value) -> Value {
    let action = text_arg(args, "action");
    let status = match env.deps.control.status() {
        Ok(s) => s,
        Err(e) => return failure(&format!("état indisponible : {e:#}")),
    };
    env.deps.audit("sources", "run_ops", json!({ "action": action, "status": status_json(&status) }));
    match action.as_str() {
        "diagnose" => {
            let mut out = status_json(&status);
            out["verdict"] = json!(verdict(&status));
            out["pseudo"] = json!(text_arg(args, "pseudo"));
            out
        }
        "sante_flux" => json!({ "chaine": status.channel.map(|c| c.name), "mesure": { "debit_source_kbps": status.health.source_kbps, "saccades": status.health.stutter, "image_noire": status.health.black, "image_figee": status.health.frozen } }),
        "fournisseurs" => json!({ "actif": status.provider, "chaine": status.channel.map(|c| c.name) }),
        other => failure(&format!("action inconnue ou indisponible : {other}")),
    }
}

pub fn request_channel(env: &mut ToolEnv, channel_id: &str, channel_name: &str, note: &str) -> Value {
    let user = env.message.user_id.clone();
    let result = env.deps.room.request_switch(channel_id, &user, note);
    env.deps.audit("decisions", "request_switch", json!({ "user": user, "channel": channel_id, "name": channel_name, "note": note, "result": format!("{result:?}") }));
    let outcome = match result {
        Ok(o) => o,
        Err(e) => return json!({ "ok": false, "status": "error", "erreur": format!("{e:#}") }),
    };
    match (outcome.outcome.as_str(), outcome.reason.as_deref()) {
        ("voteStarted", _) => json!({ "status": "vote_open", "channel": channel_name, "note": "le système a ouvert le vote dans le chat : n'ajoute rien" }),
        ("switchedDirectly", _) => json!({ "status": "switched", "channel": channel_name, "note": "le système a annoncé le changement dans le chat : n'ajoute rien" }),
        (_, Some("alreadyOn")) => json!({ "ok": false, "status": "already_on", "channel": channel_name, "raison": "c'est déjà la chaîne en cours : dis-le simplement" }),
        (_, Some("busy")) => json!({ "ok": false, "status": "busy", "channel": channel_name, "raison": "un vote est déjà ouvert : demande d'attendre son résultat" }),
        (_, Some("presenceUnavailable")) => json!({ "ok": false, "status": "presence_indisponible", "raison": "la présence est illisible, aucun vote ne peut s'ouvrir : réessaie dans un instant" }),
        (_, reason) => json!({ "ok": false, "status": "refused", "channel": channel_name, "raison": reason.unwrap_or("refusé") }),
    }
}

fn change_channel(env: &mut ToolEnv, args: &Value) -> Value {
    let reference = text_arg(args, "ref");
    let query = text_arg(args, "query");
    if reference.is_empty() && query.is_empty() {
        return failure("query ou ref est obligatoire");
    }
    let search = if reference.is_empty() { &query } else { &reference };
    let found = match env.deps.control.resolve(search, 5) {
        Ok(found) => found,
        Err(e) => return failure(&format!("catalogue indisponible : {e:#}")),
    };
    let chosen = if reference.is_empty() { found.first() } else { found.iter().find(|c| c.id == reference) };
    let Some(channel) = chosen else {
        env.deps.audit("decisions", "change_channel_refused", json!({ "query": query, "ref": reference, "why": "not found", "suggestions": found.iter().map(|c| c.name.clone()).collect::<Vec<_>>() }));
        let status = if reference.is_empty() { "not_found" } else { "ref_inconnue" };
        return json!({ "ok": false, "status": status, "suggestions": found.iter().take(5).map(|c| c.name.clone()).collect::<Vec<_>>() });
    };
    let (id, name) = (channel.id.clone(), channel.name.clone());
    request_channel(env, &id, &name, &query)
}
