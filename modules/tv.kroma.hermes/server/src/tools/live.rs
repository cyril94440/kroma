use serde_json::{json, Value};

use super::{always, channel_ref, failure, int_arg, provider_arg, provider_fr, text_arg, Kind, Spec, ToolEnv};
use crate::paris::clock_label;
use crate::peers::{Candidate, SwitchAsk, Slot, Status};

const FORCE_DESC: &str = "true UNIQUEMENT si la personne demande explicitement de forcer / passer sans vote (ex: « force le changement », « sans vote », « en admin »). Le serveur vérifie lui-même que la personne est autorisée : sinon un vote normal est lancé. Ne mets jamais true de toi-même.";
const REF_DESC: &str = "Référence d'une chaîne telle que renvoyée par un outil : « p:<n°> » (principal), « s:<n°> » (secours) ou « #S.N » (groupe N du scan S, ex. '#3.1'). Ne l'invente jamais.";
const PROVIDER_DESC: &str = "Pour un NOM seulement : le chercher dans le catalogue de ce fournisseur (défaut : le principal puis le secours)";

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "change_channel",
            kind: Kind::Action,
            activity: Some("switch"),
            available: always,
            describe: || {
                (
                    "Demande à diffuser une chaîne POUR TOUT LE MONDE : par son nom (règles maison : jamais de 4K, préférence VIP RAW) ou, mieux quand tu l'as, par sa RÉFÉRENCE (ref) — y compris une chaîne du fournisseur de SECOURS trouvée par un scan. Le système applique lui-même la règle (bascule immédiate si le demandeur est seul, vote de 30 s sinon) et l'annonce dans le chat. UNE seule action de diffusion par message. JAMAIS pour « tester » ou « voir » une chaîne : pour ça, benchmark_channels ou look_at_live. Si la chaîne est introuvable, rien n'est posté et le résultat te revient : cherche autrement (list_channels, benchmark_channels) ou réponds avec reply.",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string", "description": "Nom de la chaîne demandée, ex: 'Arte', 'Canal+ Foot' (inutile si ref)" },
                        "ref": { "type": "string", "description": REF_DESC },
                        "fournisseur": { "type": "string", "enum": ["principal", "secours"], "description": PROVIDER_DESC },
                        "force": { "type": "boolean", "description": FORCE_DESC },
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
                    "LISTER, en lecture seule, les chaînes du CATALOGUE correspondant à une recherche (instantané, aucune connexion vidéo) : nom exact, famille, fournisseur, ce qui passe en ce moment, et une référence (p:…, s:…) réutilisable avec benchmark_channels, probe_channel, change_channel ou switch_provider. Ne dit PAS si une chaîne diffuse : pour ça, benchmark_channels ou probe_channel. Les noms et les numéros diffèrent d'un fournisseur à l'autre ; `evenement` signale un multiplex d'événements (une chaîne par match).",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string", "description": "Mots-clés, ex: 'ligue 1', 'canal+ foot', 'bein'" },
                        "fournisseur": { "type": "string", "enum": ["principal", "secours"], "description": "Catalogue à lister (défaut : les deux)" },
                        "limit": { "type": "integer", "description": "Nombre max de chaînes (défaut 40, max 80)" },
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
    ]
}

pub fn slot_json(slot: &Option<Slot>) -> Value {
    slot.as_ref().map_or(Value::Null, |s| json!({ "titre": s.title, "debut": clock_label(s.start), "fin": clock_label(s.end) }))
}

fn candidate_json(c: &Candidate) -> Value {
    let mut out = json!({ "ref": c.id, "nom": c.name, "groupe": c.group, "fournisseur": provider_fr(&c.provider), "en_cours": c.now.as_ref().map(|s| s.title.clone()) });
    if let Some(event) = &c.event {
        out["evenement"] = json!({ "recherche": event.search, "competition": event.competition });
    }
    out
}

fn list_channels(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    let Ok(provider) = provider_arg(&text_arg(args, "fournisseur")) else { return failure("fournisseur : principal ou secours") };
    let limit = int_arg(args, "limit", 40).clamp(1, 80) as u32;
    match env.deps.control.resolve(&query, limit, provider) {
        Ok(found) => {
            env.deps.audit("sources", "list_channels", json!({ "query": query, "provider": provider, "count": found.len(), "top": found.iter().take(5).map(|c| json!({ "id": c.id, "name": c.name, "score": c.score, "provider": c.provider })).collect::<Vec<_>>() }));
            json!({ "total": found.len(), "chaines": found.iter().map(candidate_json).collect::<Vec<_>>() })
        }
        Err(e) => failure(&format!("catalogue indisponible : {e:#}")),
    }
}

pub fn status_json(status: &Status) -> Value {
    json!({
        "etat": status.state,
        "chaine": status.channel.as_ref().map(|c| json!({ "ref": c.id, "nom": c.name })),
        "fournisseur": provider_fr(&status.provider),
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

pub fn request_channel(env: &mut ToolEnv, channel_id: &str, channel_name: &str, note: &str, force: bool) -> Value {
    let user = env.message.user_id.clone();
    let ask = SwitchAsk { channel_id, channel_name: Some(channel_name), requested_by: &user, reason: note, force };
    let result = env.deps.room.request_switch(&ask);
    env.deps.audit("decisions", "request_switch", json!({ "user": user, "channel": channel_id, "name": channel_name, "note": note, "force": force, "result": format!("{result:?}") }));
    let outcome = match result {
        Ok(o) => o,
        Err(e) => return json!({ "ok": false, "status": "error", "erreur": format!("{e:#}") }),
    };
    match (outcome.outcome.as_str(), outcome.reason.as_deref()) {
        ("voteStarted", _) if force => json!({ "status": "vote_open", "channel": channel_name, "note": "le passage en force n'a pas été accordé (réservé aux administrateurs autorisés) : le système a ouvert un vote normal dans le chat, n'ajoute rien" }),
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
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    if reference.is_empty() && query.is_empty() {
        return failure("query ou ref est obligatoire");
    }
    let Ok(provider) = provider_arg(&text_arg(args, "fournisseur")) else { return failure("fournisseur : principal ou secours") };
    let wanted = if reference.is_empty() { None } else { channel_ref(env, &reference) };
    if !reference.is_empty() && wanted.is_none() {
        return json!({ "ok": false, "status": "ref_inconnue", "raison": "je ne retrouve pas cette chaîne (le scan est peut-être trop ancien) : redemande-la ou relance une recherche" });
    }
    let search = wanted.clone().unwrap_or_else(|| query.clone());
    let found = match env.deps.control.resolve(&search, 5, if wanted.is_some() { None } else { provider }) {
        Ok(found) => found,
        Err(e) => return failure(&format!("catalogue indisponible : {e:#}")),
    };
    let chosen = match &wanted {
        Some(id) => found.iter().find(|c| c.id == *id),
        None => found.first(),
    };
    let Some(channel) = chosen else {
        env.deps.audit("decisions", "change_channel_refused", json!({ "query": query, "ref": reference, "provider": provider, "why": "not found", "suggestions": found.iter().map(|c| c.name.clone()).collect::<Vec<_>>() }));
        let status = if wanted.is_some() { "ref_inconnue" } else { "not_found" };
        return json!({ "ok": false, "status": status, "suggestions": found.iter().take(5).map(|c| c.name.clone()).collect::<Vec<_>>() });
    };
    let (id, name) = (channel.id.clone(), channel.name.clone());
    request_channel(env, &id, &name, &query, force)
}
