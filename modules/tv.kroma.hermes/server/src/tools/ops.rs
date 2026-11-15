use serde_json::{json, Value};

use super::chart::{abnormal, episodes};
use super::live::status_json;
use super::{failure, provider_fr, text_arg, Kind, Spec, ToolEnv};
use crate::paris::clock_label;
use crate::peers::{HistoryPoint, Status};

const HISTORY_POINTS: u32 = 180;
const RECENT_POINTS: usize = 60;
const PLAYER_FREEZES_MIN: usize = 2;

pub fn is_restart(args: &Value) -> bool {
    matches!(text_arg(args, "action").as_str(), "restart_ingest" | "ingest_restart")
}

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "run_ops",
        kind: Kind::Investigation,
        activity: Some("status"),
        available: super::always,
        describe: || {
            (
                "Dépannage flux (lecture seule, sauf restart_ingest). 'diagnose' = état du flux + historique récent (relances, épisodes de panne) + verdict global ; passe 'pseudo' (qui signale le souci) pour croiser ses gels vidéo côté client. 'fournisseurs' = fournisseur actif et dernières bascules / pannes constatées. 'sante_flux' = détail des saccades, écran noir, image figée, débit du flux en cours. 'restart_ingest' = relance propre du flux (~5 s de coupure pour TOUT LE MONDE) : le serveur la REFUSE s'il ne constate lui-même aucun souci (flux arrêté, sans débit, écran noir, image figée, saccades côté source), sauf si le demandeur est seul — explique alors et n'insiste pas. À utiliser quand on signale saccades, coupures, gel, écran noir, ou qu'une chaîne « ne répond pas ».",
                json!({ "type": "object", "properties": {
                    "action": { "type": "string", "enum": ["diagnose", "fournisseurs", "sante_flux", "restart_ingest"] },
                    "pseudo": { "type": "string", "description": "Pseudo de la personne qui signale le souci (pour 'diagnose' uniquement, optionnel)" },
                }, "required": ["action"] }),
            )
        },
        handler: run_ops,
    }]
}

fn unstable(status: &Status, restarts: i64) -> bool {
    status.health.stutter || status.state == "error" || restarts >= 2
}

fn verdict(status: &Status, restarts: i64, freezes: Option<(usize, &str)>) -> String {
    if status.state == "offAir" {
        return "chaine_hors_antenne : la chaîne ne diffuse plus rien chez le fournisseur ; propose une alternative (list_channels) sans changer toi-même.".into();
    }
    if status.health.black || status.health.frozen {
        return "image_noire_ou_figee : le flux arrive mais l'image est noire ou figée ; si c'est un multiplex, vérifie avec look_at_live puis cherche le programme ailleurs ; un restart_ingest peut aider en cas de gel isolé.".into();
    }
    if unstable(status, restarts) {
        let mut text = "source_instable : le flux du fournisseur se coupe ou saccade. Tout le monde reçoit le même flux : ce n'est pas l'appareil de la personne. Un reboot n'aidera pas durablement ; conseille de patienter ou de changer de chaîne.".to_string();
        if status.provider == "primary" {
            text.push_str(" Un fournisseur de SECOURS peut exister : propose de basculer dessus (switch_provider, même chaîne) si ça persiste.");
        }
        return text;
    }
    if status.health.source_kbps > 0.0 {
        return match freezes {
            Some((count, who)) if count >= PLAYER_FREEZES_MIN => format!("cote_spectateur : le flux est stable côté serveur, mais {who} a eu {count} gels vidéo dans les 30 dernières minutes — le souci est chez lui/elle : conseille de recharger la page ou de vérifier son wifi."),
            _ => "serveur_stable : le flux est sain côté serveur. Les saccades viennent probablement de la connexion ou de l'appareil du spectateur (conseille de recharger la page, vérifier le wifi).".into(),
        };
    }
    "indetermine : pas de mesure de débit disponible pour l'instant ; un redémarrage propre (restart_ingest) peut aider en cas de gel isolé.".into()
}

fn recent_points(env: &ToolEnv, limit: u32) -> Vec<HistoryPoint> {
    env.deps.control.history(limit).map(|h| h.points).unwrap_or_default()
}

fn restarts_recent(points: &[HistoryPoint]) -> i64 {
    let tail = &points[points.len().saturating_sub(RECENT_POINTS)..];
    match (tail.first(), tail.last()) {
        (Some(first), Some(last)) => (last.restarts - first.restarts).max(0),
        _ => 0,
    }
}

fn player_freezes(env: &ToolEnv, status: &Status, pseudo: &str) -> Option<(usize, String)> {
    let viewer = status.viewers.iter().find(|v| crate::webcache::normalise(&v.name) == crate::webcache::normalise(pseudo))?;
    let events = env.deps.room.client_logs(Some(&viewer.user_id), 200, Some(30)).ok()?;
    let count = events.iter().filter(|e| e.get("event").and_then(Value::as_str).is_some_and(|n| n.contains("gel") || n.contains("freeze"))).count();
    Some((count, viewer.name.clone()))
}

fn diagnose(env: &mut ToolEnv, status: &Status, pseudo: &str) -> Value {
    let points = recent_points(env, HISTORY_POINTS);
    let restarts = restarts_recent(&points);
    let freezes = if pseudo.is_empty() { None } else { player_freezes(env, status, pseudo) };
    let mut out = status_json(status);
    out["verdict"] = json!(verdict(status, restarts, freezes.as_ref().map(|(n, who)| (*n, who.as_str()))));
    out["pseudo"] = json!(pseudo);
    if !points.is_empty() {
        out["historique"] = json!({ "relances_10_min": restarts, "episodes_30_min": episodes(&points[points.len().saturating_sub(180)..]), "points": points.len() });
    }
    if let Some((count, who)) = freezes {
        out["spectateur"] = json!({ "pseudo": who, "gels_30_min": count });
    }
    out
}

fn sante_flux(status: &Status, points: &[HistoryPoint]) -> Value {
    let tail = &points[points.len().saturating_sub(6)..];
    json!({
        "chaine": status.channel.as_ref().map(|c| c.name.clone()),
        "mesure": { "debit_source_kbps": status.health.source_kbps, "saccades": status.health.stutter, "image_noire": status.health.black, "image_figee": status.health.frozen },
        "detail_saccades": tail.iter().map(|p| json!({ "a": clock_label(p.at * 1000), "debit_kbps": p.kbps, "perte_60s": p.loss, "sante": p.health, "anormal": abnormal(p) })).collect::<Vec<_>>(),
    })
}

fn fournisseurs(status: &Status, events: &[Value]) -> Value {
    json!({
        "actif": provider_fr(&status.provider),
        "chaine": status.channel.as_ref().map(|c| c.name.clone()),
        "evenements_recents": events.iter().rev().take(10).map(|e| json!({
            "genre": e.get("kind"), "de": e.get("from"), "vers": e.get("to"), "chaine": e.get("channel"), "detail": e.get("detail"),
            "a": e.get("at").and_then(Value::as_i64).map(clock_label),
        })).collect::<Vec<_>>(),
    })
}

fn restart(env: &mut ToolEnv, status: &Status, pseudo: &str) -> Value {
    let user = env.message.user_id.clone();
    let name = status.channel.as_ref().map(|c| c.name.clone()).unwrap_or_default();
    let result = env.deps.control.restart(&user, &format!("restart_ingest demandé par {}", env.message.name));
    env.deps.audit("decisions", "restart_ingest", json!({ "user": user, "pseudo": pseudo, "channel": name, "result": format!("{result:?}") }));
    let outcome = match result {
        Ok(o) => o,
        Err(e) => return failure(&format!("relance impossible : {e:#}")),
    };
    if outcome.restarted {
        return json!({ "ok": true, "message": format!("Flux relancé proprement sur {name} (~5 s de coupure)."), "chaine": name });
    }
    let (raison, message) = match outcome.error.as_deref() {
        Some("noChannel") => ("aucune_chaine", "Aucune chaîne à relancer."),
        Some("standby") => ("veille", "Le flux est en veille (personne ne regarde) : rien à relancer."),
        Some("streamHealthy") => ("flux_sain", "Le flux est sain côté serveur : une relance couperait l'image de tout le monde pour rien. Explique-le, n'insiste pas."),
        Some("recentRestart") => ("relance_recente", "Le flux vient déjà d'être relancé il y a moins de 5 min."),
        Some("channelChanged") => ("chaine_changee", "La chaîne vient de changer (bascule en cours) : rien à relancer, regarde où on en est avant d'agir."),
        other => ("refuse", other.unwrap_or("refusé")),
    };
    json!({ "ok": false, "raison": raison, "message": message, "constat": status_json(status)["sante"] })
}

fn run_ops(env: &mut ToolEnv, args: &Value) -> Value {
    let action = text_arg(args, "action");
    let status = match env.deps.control.status() {
        Ok(s) => s,
        Err(e) => return failure(&format!("état indisponible : {e:#}")),
    };
    env.deps.audit("sources", "run_ops", json!({ "action": action, "status": status_json(&status) }));
    match action.as_str() {
        "diagnose" | "ingest_status" => diagnose(env, &status, &text_arg(args, "pseudo")),
        "sante_flux" => sante_flux(&status, &recent_points(env, 6)),
        "fournisseurs" => fournisseurs(&status, &env.deps.control.history(1).map(|h| h.provider_events).unwrap_or_default()),
        "restart_ingest" | "ingest_restart" => restart(env, &status, &text_arg(args, "pseudo")),
        other => failure(&format!("action inconnue ou indisponible : {other}")),
    }
}
