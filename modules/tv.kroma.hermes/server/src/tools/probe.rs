use serde_json::{json, Value};

use super::{channel_ref, failure, method_missing, provider_arg, provider_fr, text_arg, too_late, Availability, Kind, Spec, ToolEnv};
use crate::peers::Probe;

pub const NO_TEST_REASON: &str = "non testée : le direct occupe la connexion du fournisseur de secours (seule la capture d'un scan peut s'ouvrir à côté)";

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "probe_channel",
        kind: Kind::Investigation,
        activity: Some("probe"),
        available: |a: &Availability| a.on("probe"),
        describe: || {
            (
                "Analyse TECHNIQUE d'une chaîne sans changer la diffusion, chez le principal OU le secours : codec vidéo (H264/HEVC), résolution, fps, débit réel mesuré (mbps_mesure) et audio — ou la RAISON précise d'un échec. Pour comparer la qualité de 2 chaînes, appelle-le une fois PAR chaîne (~10 à 25 s chacune) puis fais la synthèse avec reply. Ex: « quel flux est meilleur entre BeIN et M6 ? » -> probe_channel('bein sports 1') puis probe_channel('M6') puis reply(comparaison chiffrée). « occupe » = connexions de test toutes prises : ce n'est PAS une chaîne morte.",
                json!({ "type": "object", "properties": {
                    "query": { "type": "string", "description": "Nom de la chaîne (ex: 'BeIN Sports 1', 'M6') ou une référence (p:…, s:…, #S.N)" },
                    "fournisseur": { "type": "string", "enum": ["principal", "secours"], "description": "Pour un nom : chez quel fournisseur (défaut : le principal puis le secours)" },
                }, "required": ["query"] }),
            )
        },
        handler: probe_channel,
    }]
}

fn is_ref(text: &str) -> bool {
    let t = text.trim();
    t.starts_with('#') || t.split_once(':').is_some_and(|(p, n)| matches!(p, "p" | "s") && !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

fn target(env: &mut ToolEnv, query: &str, provider_text: &str) -> Result<(String, String), Value> {
    let Ok(provider) = provider_arg(provider_text) else { return Err(failure("fournisseur : principal ou secours")) };
    if is_ref(query) {
        let Some(id) = channel_ref(env, query) else { return Err(json!({ "error": format!("référence inconnue : {query} (scan trop ancien ?)") })) };
        let found = env.deps.control.resolve(&id, 1, None).map_err(|e| failure(&format!("catalogue indisponible : {e:#}")))?;
        return found.into_iter().find(|c| c.id == id).map(|c| (c.id, c.name)).ok_or_else(|| json!({ "error": format!("référence inconnue : {query} (scan trop ancien ?)") }));
    }
    let found = env.deps.control.resolve(query, 5, provider).map_err(|e| failure(&format!("catalogue indisponible : {e:#}")))?;
    found.first().map(|c| (c.id.clone(), c.name.clone())).ok_or_else(|| json!({ "error": format!("chaîne « {query} » introuvable"), "suggestions": [] }))
}

fn describe(report: &Probe, id: &str, name: &str) -> Value {
    let base = json!({ "ref": id, "chaine": name, "fournisseur": report.provider.as_deref().map(provider_fr) });
    let with = |extra: Value| {
        let mut out = base.clone();
        for (k, v) in extra.as_object().cloned().unwrap_or_default() {
            out[k] = v;
        }
        out
    };
    match report.error.as_deref() {
        Some("busy") => return with(json!({ "etat": "occupe", "raison": "connexions de test toutes prises (un scan est en cours ?) — ce n'est PAS une chaîne morte" })),
        Some("liveAtThisProvider") => return with(json!({ "etat": "refus", "raison": NO_TEST_REASON })),
        Some("standby") => return with(json!({ "etat": "veille", "raison": "le flux est en veille : personne ne regarde, la sonde locale n'est pas possible" })),
        Some("notReady") => return with(json!({ "etat": "indisponible", "raison": "le flux démarre : la sonde locale n'est pas encore possible, réessaie dans quelques secondes" })),
        Some("unknownChannel") => return json!({ "error": format!("référence inconnue : {id}") }),
        Some(other) => return with(json!({ "error": format!("sonde impossible : {other}") })),
        None => {}
    }
    if !report.alive {
        return with(json!({ "etat": "ne_repond_pas", "vivante": false, "hors_antenne": report.off_air, "raison": "la chaîne ne renvoie rien chez le fournisseur en ce moment" }));
    }
    let mut out = with(json!({ "vivante": true, "codec": report.codec, "resolution": report.resolution, "codec_audio": report.audio_codec, "hors_antenne": report.off_air }));
    if let Some(fps) = report.fps {
        out["fps"] = json!(fps);
    }
    if let Some(kbps) = report.bitrate_kbps {
        out["debit_kbps"] = json!(kbps);
        out["mbps_mesure"] = json!((kbps / 100.0).round() / 10.0);
    }
    if report.local {
        out["note"] = json!("chaîne en cours de diffusion (mesure locale, audio réencodé en AAC par notre serveur)");
    }
    out
}

fn probe_channel(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    if let Some(late) = too_late(env, "une sonde") {
        return late;
    }
    let (id, name) = match target(env, &query, &text_arg(args, "fournisseur")) {
        Ok(found) => found,
        Err(refusal) => return refusal,
    };
    env.deps.audit("sources", "probe_channel", json!({ "query": query, "channel": name, "id": id }));
    match env.deps.control.probe(&id) {
        Ok(report) => {
            env.deps.audit("sources", "probe_report", json!({ "id": id, "alive": report.alive, "error": report.error, "codec": report.codec, "resolution": report.resolution, "kbps": report.bitrate_kbps }));
            describe(&report, &id, &name)
        }
        Err(e) => {
            method_missing(env, "probe", &e);
            json!({ "ref": id, "chaine": name, "error": format!("sonde impossible : {e:#}") })
        }
    }
}
