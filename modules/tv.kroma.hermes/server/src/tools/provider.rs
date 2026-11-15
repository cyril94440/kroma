use serde_json::{json, Value};

use super::live::request_channel;
use super::{channel_ref, failure, method_missing, provider_arg, provider_fr, text_arg, Availability, Kind, Spec, ToolEnv};
use crate::matching::Verdict;
use crate::peers::{Candidate, ProviderSwitch, Status};
use crate::scans::LIVE_MEMORY_MS;

const FORCE_DESC: &str = "true UNIQUEMENT si la personne demande explicitement de forcer / passer sans vote. Le serveur vérifie lui-même que la personne y a droit : sinon un vote normal est lancé. Ne mets jamais true de toi-même.";

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "switch_provider",
        kind: Kind::Action,
        activity: Some("provider"),
        available: |a: &Availability| a.on("switchProvider"),
        describe: || {
            (
                "Changer de FOURNISSEUR IPTV en gardant le MÊME programme : « passe sur le secours », « essaie l'autre fournisseur / l'autre source », « reviens sur le principal ». Sans ref, le système prend la même chaîne chez l'autre fournisseur (même nom ou même numéro, vérifiée qu'elle répond). ⚠️ Pour un multiplex (Ligue1+, DAZN…), les numéros ne se correspondent PAS d'un fournisseur à l'autre : si tu as repéré la bonne chaîne avec benchmark_channels (comparer_au_direct=true), passe sa référence dans ref. Le système applique la règle (seul = immédiat ; souci constaté par le serveur = immédiat ; sinon vote) et l'annonce lui-même. Si la chaîne ne répond pas ou n'existe pas, RIEN n'est posté : le résultat te revient avec la raison et la suite possible (suite_possible) — enquête avant de répondre. Ne sert PAS à changer de chaîne chez le même fournisseur.",
                json!({ "type": "object", "properties": {
                    "target": { "type": "string", "description": "Fournisseur voulu : « principal » ou « secours » (« autre » = celui qui n'est pas actif). Ignoré si ref est donné." },
                    "ref": { "type": "string", "description": "Référence (p:…, s:…, #S.N) d'une chaîne de l'AUTRE fournisseur, trouvée par un outil. Ne l'invente jamais." },
                    "force": { "type": "boolean", "description": FORCE_DESC },
                } }),
            )
        },
        handler: switch_provider,
    }]
}

fn target_of(text: &str, status: &Status) -> &'static str {
    let t = text.to_lowercase();
    match provider_arg(&t) {
        Ok(Some(p)) => p,
        _ if status.provider == "backup" => "primary",
        _ => "backup",
    }
}

fn current(env: &ToolEnv, status: &Status) -> Option<Candidate> {
    let id = status.channel.as_ref()?.id.clone();
    env.deps.control.resolve(&id, 1, None).ok()?.into_iter().find(|c| c.id == id)
}

fn look_suite(candidate: &Option<Candidate>, target: &str) -> Option<String> {
    let event = candidate.as_ref()?.event.as_ref()?;
    Some(format!("benchmark_channels(query='{}', fournisseur='{}', comparer_au_direct=true)", event.search, provider_fr(target)))
}

fn flag_multiplex(mut out: Value, candidate: &Option<Candidate>, target: &str) -> Value {
    if let Some(suite) = look_suite(candidate, target) {
        out["multiplex"] = json!(true);
        out["suite_possible"] = json!(suite);
    }
    out
}

fn by_target(env: &mut ToolEnv, status: &Status, target: &'static str, force: bool) -> Value {
    let user = env.message.user_id.clone();
    let now_on = current(env, status);
    let result = env.deps.control.switch_provider(target, &user, &format!("switch_provider demandé par {}", env.message.name));
    env.deps.audit("decisions", "switch_provider", json!({ "user": user, "target": target, "force": force, "result": format!("{result:?}") }));
    let ProviderSwitch { switched, error, .. } = match result {
        Ok(r) => r,
        Err(e) => {
            if method_missing(env, "switchProvider", &e) {
                return failure("changement de fournisseur indisponible : le module live ne le propose pas");
            }
            return failure(&format!("changement de fournisseur impossible : {e:#}"));
        }
    };
    let label = provider_fr(target);
    let mut out = match error.as_deref() {
        None if switched => {
            env.ctx.diffusion_done = true;
            json!({ "status": "switched_provider", "provider": label, "note": "le flux repart chez ce fournisseur (~5 s de coupure) : dis-le en UNE phrase courte" })
        }
        Some("voteRequired") => {
            env.ctx.diffusion_done = true;
            json!({ "status": "vote_open", "provider": label, "note": "le système a ouvert le vote dans le chat : n'ajoute rien" })
        }
        Some("noBackup") => json!({ "ok": false, "status": "no_backup", "raison": "il n'y a pas de fournisseur de secours configuré" }),
        Some("noChannel") => json!({ "ok": false, "status": "no_channel", "raison": "aucune chaîne en cours : demande d'abord une chaîne" }),
        Some("alreadyOn") => json!({ "ok": false, "status": "already_on", "provider": label, "raison": format!("on est déjà sur le fournisseur {label}") }),
        Some("primaryDown") => json!({ "ok": false, "status": "primary_down", "raison": "le fournisseur principal est toujours en panne : on reste sur le secours, la famille est prévenue dès qu'il est rétabli" }),
        Some("notFound") => json!({ "ok": false, "status": "not_found", "raison": format!("cette chaîne n'existe pas chez le fournisseur {label}") }),
        Some("notWorking") => json!({ "ok": false, "status": "dead", "raison": format!("la chaîne ne répond pas chez le fournisseur {label} en ce moment (testée à l'instant) : on reste comme on est") }),
        Some("busy") => json!({ "ok": false, "status": "occupe", "raison": format!("les connexions de test vers le fournisseur {label} sont toutes prises (une recherche est en cours) : redemande dans une minute") }),
        Some(other) => json!({ "ok": false, "status": "refused", "raison": other }),
    };
    if matches!(error.as_deref(), Some("notFound" | "notWorking" | "voteRequired")) {
        out = flag_multiplex(out, &now_on, target);
    }
    if force {
        out["note_force"] = json!("le passage en force n'existe pas pour un simple changement de fournisseur : la règle (seul, souci constaté, sinon vote) s'applique");
    }
    out
}

fn contradicted(env: &ToolEnv, live: &str, ref_teams: &[String], ref_at: Option<i64>, now: i64) -> bool {
    let Some(seen) = env.deps.live_seen.teams(live, now, LIVE_MEMORY_MS).filter(|s| s.at > ref_at.unwrap_or(0)) else { return false };
    let mut shown = crate::scans::team_names(&seen.reading);
    let mut wanted = ref_teams.to_vec();
    shown.sort();
    wanted.sort();
    !shown.is_empty() && shown != wanted
}

fn by_ref(env: &mut ToolEnv, status: &Status, reference: &str, force: bool) -> Value {
    let Some(id) = channel_ref(env, reference) else { return json!({ "ok": false, "status": "ref_inconnue", "raison": "je ne retrouve pas cette chaîne (le scan est peut-être trop ancien) : redemande-la ou relance une recherche" }) };
    let picked = match env.deps.control.resolve(&id, 1, None) {
        Ok(found) => found.into_iter().find(|c| c.id == id),
        Err(e) => return failure(&format!("catalogue indisponible : {e:#}")),
    };
    let Some(picked) = picked else { return json!({ "ok": false, "status": "ref_inconnue", "raison": "je ne retrouve pas cette chaîne (le scan est peut-être trop ancien) : redemande-la ou relance une recherche" }) };
    if picked.provider == status.provider {
        return json!({ "ok": false, "status": "meme_fournisseur", "raison": format!("{} est chez le fournisseur déjà actif : c'est un changement de chaîne, pas de fournisseur", picked.name), "suite_possible": format!("change_channel(ref='{id}')") });
    }
    let (Some(live), now) = (status.channel.as_ref().map(|c| c.id.clone()), env.deps.now()) else { return json!({ "ok": false, "status": "no_channel", "raison": "aucune chaîne en cours : demande d'abord une chaîne" }) };
    let judged = env.deps.scans.judged(&id, &live, now);
    if let Some(j) = &judged {
        if j.verdict == Verdict::No && now - j.scan_at <= LIVE_MEMORY_MS && !contradicted(env, &live, &j.ref_teams, j.ref_at, now) {
            env.deps.audit("decisions", "switch_provider_other_match", json!({ "channel": picked.name, "scan_at": j.scan_at }));
            return json!({ "ok": false, "status": "autre_match", "raison": format!("{} ne montre pas le même programme que le direct (vérifié à l'image) : je ne bascule pas dessus. Si la famille veut la regarder, c'est un changement de chaîne.", picked.name), "suite_possible": format!("change_channel(ref='{id}')") });
        }
    }
    let checked = judged.as_ref().is_some_and(|j| j.verdict == Verdict::Yes);
    let note = format!("fournisseur {} : {} ({})", provider_fr(&picked.provider), picked.name, if checked { "même match vérifié à l'image" } else { "non vérifié à l'image" });
    env.deps.audit("decisions", "switch_provider_ref", json!({ "channel": picked.name, "verified": checked, "force": force }));
    let mut out = request_channel(env, &id, &picked.name, &note, force);
    out["verifie_a_l_image"] = json!(checked);
    out
}

fn switch_provider(env: &mut ToolEnv, args: &Value) -> Value {
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    let reference = text_arg(args, "ref");
    let status = match env.deps.control.status() {
        Ok(s) => s,
        Err(e) => return failure(&format!("état indisponible : {e:#}")),
    };
    if !reference.is_empty() {
        return by_ref(env, &status, &reference, force);
    }
    let target = target_of(&text_arg(args, "target"), &status);
    by_target(env, &status, target, force)
}
