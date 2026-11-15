use serde_json::{json, Value};

use super::{channel_ref, failure, int_arg, method_missing, provider_arg, provider_fr, text_arg, Availability, Kind, Spec, ToolEnv};
use crate::strings;

mod follow;
mod report;

pub use follow::Follow;

const MAX_SCANS_PER_TURN: u32 = 2;
const MAX_IDS: usize = 8;
const SCAN_MIN_TIME_S: f64 = 50.0;
const SCAN_TIME_S: f64 = 120.0;
const PROVIDER_DESC: &str = "Fournisseur à scanner (défaut : le fournisseur actif)";

#[derive(Debug, Clone)]
pub struct ScanJob {
    pub id: String,
    pub query: String,
    pub provider: String,
    pub compare: bool,
    pub search: String,
    pub live: Option<String>,
    pub by: String,
    pub started_at: i64,
    pub budget_ms: i64,
    pub total: usize,
    pub already_done: bool,
}

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "benchmark_channels",
        kind: Kind::Investigation,
        activity: Some("scan"),
        available: |a: &Availability| a.on("benchmark"),
        describe: || {
            (
                "SCANNER plusieurs chaînes d'UN fournisseur (principal OU secours) pour voir ce qu'elles diffusent RÉELLEMENT : une capture courte par chaîne + analyse d'image, SANS couper ni changer la diffusion en cours. C'est l'outil pour TESTER des chaînes, lever l'ambiguïté d'un multiplex (« ligue1+ » = 10 chaînes numérotées, une par match, souvent noires) et RETROUVER le match qu'on regarde chez l'autre fournisseur : avec comparer_au_direct=true, chaque chaîne est comparée à ce que voit la famille (équipes, score, horloge lus à l'image ; c'est le SERVEUR qui tranche oui / probable / non / inconnu) ; avec cherche='Lens - Paris FC', il signale les chaînes qui montrent ce match (utile si le direct est noir ou si le guide ne connaît pas le match). Les numéros de multiplex ne se correspondent PAS d'un fournisseur à l'autre : seul le contenu compte. Huit chaînes au plus par scan ; le serveur limite lui-même les connexions (le secours en autorise peu). Prend ≈ 20 à 75 s, annoncé dans le chat. Renvoie des GROUPES numérotés (un contenu = un groupe) avec leur référence (ex. '#3.1' = groupe 1 du scan 3) : « mets la 1 » -> change_channel(ref='#3.1'), ou switch_provider(ref='#3.1') si la chaîne est chez l'autre fournisseur. Distingue un vrai DIRECT (horloge/score visibles) d'un résumé ou d'une bande promo. Un scan à la fois, deux par message au plus, jamais en boucle : sers-t'en dès que ça aide à répondre ou à dépanner.",
                json!({ "type": "object", "properties": {
                    "query": { "type": "string", "description": "Famille de chaînes à scanner, ex: 'ligue1+', 'dazn ligue 1', 'bein sports' (inutile si refs)" },
                    "fournisseur": { "type": "string", "enum": ["principal", "secours"], "description": PROVIDER_DESC },
                    "comparer_au_direct": { "type": "boolean", "description": "true = comparer chaque chaîne à ce que la famille regarde (pour retrouver le même match ailleurs)" },
                    "cherche": { "type": "string", "description": "Match ou programme recherché, ex: 'Lens - Paris FC' (quand le direct est noir ou sans rapport)" },
                    "refs": { "type": "array", "items": { "type": "string" }, "description": "Références précises à tester à la place de query (8 max, un seul fournisseur)" },
                    "limit": { "type": "integer", "description": "Nombre de chaînes testées (défaut 8, max 8)" },
                } }),
            )
        },
        handler: benchmark_channels,
    }]
}

fn refusal(reason: &str) -> Value {
    json!({ "status": "refus", "raison": reason })
}

type Found = Vec<(String, String, String)>;

fn candidates(env: &mut ToolEnv, args: &Value, refs: &[String], query: &str) -> Result<(Found, String), Value> {
    if !refs.is_empty() {
        let mut found: Found = Vec::new();
        for reference in refs {
            let unknown = || json!({ "error": format!("référence inconnue : {reference} (scan trop ancien ?)") });
            let id = channel_ref(env, reference).ok_or_else(unknown)?;
            let channel = env.deps.control.resolve(&id, 1, None).map_err(|e| failure(&format!("catalogue indisponible : {e:#}")))?.into_iter().find(|c| c.id == id).ok_or_else(unknown)?;
            found.push((channel.id, channel.name, channel.provider));
        }
        if found.iter().any(|c| c.2 != found[0].2) {
            return Err(json!({ "error": "références de deux fournisseurs différents : un scan ne porte que sur un fournisseur" }));
        }
        let provider = found[0].2.clone();
        return Ok((found, provider));
    }
    let Ok(wanted) = provider_arg(&text_arg(args, "fournisseur")) else { return Err(failure("fournisseur : principal ou secours")) };
    let provider = match wanted {
        Some(p) => p.to_string(),
        None => env.deps.control.status().ok().map(|s| s.provider).filter(|p| !p.is_empty()).unwrap_or_else(|| "primary".into()),
    };
    let limit = int_arg(args, "limit", MAX_IDS as i64).clamp(1, MAX_IDS as i64) as u32;
    let found = env.deps.control.resolve(query, limit, Some(&provider)).map_err(|e| failure(&format!("catalogue indisponible : {e:#}")))?;
    if found.is_empty() {
        return Err(json!({ "error": format!("aucune chaîne pour « {query} » chez le fournisseur {}", provider_fr(&provider)), "suggestions": [] }));
    }
    Ok((found.into_iter().map(|c| (c.id, c.name, c.provider)).collect(), provider))
}

fn announce(env: &mut ToolEnv, job: &ScanJob) {
    if env.ctx.progress_posted {
        return;
    }
    let eta = if job.provider == "backup" { 4 } else { 3 };
    let high = 25 + job.total as i64 * eta + if job.compare || !job.search.is_empty() { 10 } else { 0 };
    let language = env.deps.settings().language;
    let extra = if job.compare { strings::text(&language, "hermes.scan.compare", &[]) } else { String::new() };
    let query = if job.query.is_empty() { "choisies".to_string() } else { job.query.clone() };
    let text = strings::text(&language, "hermes.scan.announce", &[("count", &job.total.to_string()), ("query", &query), ("provider", provider_fr(&job.provider)), ("extra", &extra), ("seconds", &((high + 4) / 5 * 5).to_string())]);
    if env.deps.room.say(&text, None, Some(&env.message.id)).is_ok() {
        env.ctx.progress_posted = true;
    }
}

fn start_refusal(start: &crate::peers::BenchmarkStart) -> Value {
    match start.reason.as_deref() {
        Some("scanRunning") => json!({ "status": "occupe", "note": "une recherche tourne déjà : dis-le, n'en relance pas une autre" }),
        Some("notViewing") => json!({ "status": "refus", "raison": "demandeur_absent", "message": "le scan n'est ouvert qu'aux personnes qui regardent la télé en ce moment : dis-le, n'insiste pas" }),
        Some("mixedProviders") => json!({ "error": "chaînes de deux fournisseurs différents : un scan ne porte que sur un fournisseur" }),
        Some("unknownChannel") => json!({ "error": "une des chaînes est inconnue du catalogue" }),
        Some("noChannel") => json!({ "error": "aucune chaîne à scanner" }),
        other => refusal(other.unwrap_or("scan refusé")),
    }
}

fn benchmark_channels(env: &mut ToolEnv, args: &Value) -> Value {
    if env.ctx.scans >= MAX_SCANS_PER_TURN {
        return refusal("deux scans maximum par message");
    }
    if env.ctx.pending.is_some() {
        return refusal("un seul scan à la fois");
    }
    let refs: Vec<String> = args.get("refs").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).filter(|r| !r.trim().is_empty()).take(MAX_IDS).map(str::to_string).collect();
    let query = text_arg(args, "query");
    if refs.is_empty() && query.is_empty() {
        return json!({ "error": "précise la famille de chaînes à scanner (query) ou des références (refs)" });
    }
    if let Some(left) = env.ctx.left_s.filter(|left| *left < SCAN_MIN_TIME_S) {
        return refusal(&format!("plus assez de temps dans ce message (≈ {} s) pour un scan : dis ce que tu as déjà trouvé et propose de relancer la recherche", left.max(0.0) as i64));
    }
    env.ctx.scans += 1;
    let (found, provider) = match candidates(env, args, &refs, &query) {
        Ok(found) => found,
        Err(out) => return out,
    };
    let live = env.deps.control.status().ok().and_then(|s| s.channel).map(|c| c.id);
    let ids: Vec<String> = found.iter().map(|c| c.0.clone()).collect();
    let user = env.message.user_id.clone();
    let result = env.deps.control.benchmark(&ids, &user);
    env.deps.audit("decisions", "benchmark_start", json!({ "user": user, "query": query, "refs": refs, "provider": provider, "ids": ids, "result": format!("{result:?}") }));
    let start = match result {
        Ok(start) => start,
        Err(e) if method_missing(env, "benchmark", &e) => return failure("scan indisponible : le module live ne le propose pas"),
        Err(e) => return failure(&format!("scan impossible : {e:#}")),
    };
    let Some(id) = start.id.clone().filter(|_| start.status == "started") else { return start_refusal(&start) };
    let left = env.ctx.left_s.map_or(SCAN_TIME_S, |left| SCAN_TIME_S.min(left - 20.0));
    let job = ScanJob { id, query, provider, compare: args.get("comparer_au_direct").and_then(Value::as_bool).unwrap_or(false), search: text_arg(args, "cherche"), live, by: user, started_at: env.deps.now(), budget_ms: (left * 1000.0) as i64, total: start.total.max(found.len()), already_done: start.already_done };
    if !job.already_done {
        announce(env, &job);
    }
    env.ctx.pending = Some(job);
    json!({ "status": "scan_en_cours" })
}
