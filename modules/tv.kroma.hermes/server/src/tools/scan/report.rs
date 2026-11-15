use serde_json::{json, Value};

use super::follow::{Follow, Reference};
use super::ScanJob;
use crate::matching::{self, Verdict};
use crate::scans::{Group, RowState, Scan};
use crate::tools::provider_fr;

const NOTE: &str = "Présente les GROUPES un par ligne : « 1️⃣ <contenu> (<score>, <type>) — <nom de la 1re chaîne> », les correspondances d'abord (oui = même match vérifié à l'image ; probable = à confirmer). Pour en mettre une : change_channel(ref=<réf du groupe>), ou switch_provider(ref=<réf du groupe>) si elle est chez l'autre fournisseur — n'affiche jamais les références. Un résumé ou une bande promo n'est PAS un direct. Résume les chaînes noires ou sans réponse en une ligne, avec leur raison si elle est donnée. N'invente aucun contenu.";
const PORTAL_ERRORS: [&str; 3] = ["erreur du portail (5XX)", "refus du portail (403)", "introuvable chez le portail (404)"];

pub fn classify_error(text: &str) -> String {
    let t = text.to_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| t.contains(n));
    let reason = if any(&["occup", "busy"]) {
        "non testée (connexions occupées)"
    } else if any(&["403", "forbidden"]) {
        "refus du portail (403)"
    } else if any(&["404", "not found"]) {
        "introuvable chez le portail (404)"
    } else if any(&["500", "502", "503", "504", "509", "5xx", "server error"]) {
        "erreur du portail (5XX)"
    } else if any(&["timeout", "timed out", "délai"]) {
        "délai dépassé"
    } else if any(&["aucune image", "invalid data"]) {
        "pas d'image décodée"
    } else {
        "échec de connexion"
    };
    reason.into()
}

fn group_json(scan: &Scan, group: &Group) -> Value {
    let mut out = json!({
        "n": group.n, "ref": format!("#{}.{}", scan.id, group.n), "type": group.kind, "libelle": group.label, "contenu": group.content,
        "chaines": group.members.iter().map(|m| json!({ "ref": m.id, "chaine": m.name })).collect::<Vec<_>>(),
    });
    for (key, value) in [("score", &group.score), ("horloge", &group.clock)] {
        if !value.is_empty() {
            out[key] = json!(value);
        }
    }
    if !group.teams.is_empty() {
        out["equipes"] = json!(group.teams);
    }
    if let Some(verdict) = group.verdict {
        out["meme_que_direct"] = json!(verdict.label());
        out["pourquoi"] = json!(group.why);
    }
    if let Some(target) = group.target {
        out["correspond_a_la_recherche"] = json!(target.label());
        out["pourquoi_recherche"] = json!(group.target_why);
    }
    out
}

fn blind(scan: &Scan) -> Option<Value> {
    let rows: Vec<_> = scan.rows.iter().filter(|r| r.state != RowState::Ok).collect();
    if rows.is_empty() {
        return None;
    }
    let count = |state: RowState| rows.iter().filter(|r| r.state == state).count();
    Some(json!({
        "noires": count(RowState::Black), "sans_reponse": count(RowState::Error), "non_testees": count(RowState::NotTested),
        "detail": rows.iter().take(8).map(|r| json!({ "chaine": r.name, "etat": r.state.label(), "raison": r.reason })).collect::<Vec<_>>(),
    }))
}

fn warning(scan: &Scan, provider: &str) -> Option<String> {
    let tested = scan.rows.iter().filter(|r| matches!(r.state, RowState::Ok | RowState::Black | RowState::Error)).count();
    if tested < 3 || scan.rows.iter().any(|r| r.state == RowState::Ok) {
        return None;
    }
    let reasons: Vec<&str> = scan.rows.iter().filter(|r| r.state == RowState::Error).map(|r| r.reason.as_str()).collect();
    let label = provider_fr(provider);
    if !reasons.is_empty() && reasons.iter().all(|r| PORTAL_ERRORS.contains(r)) {
        let mut unique: Vec<&str> = reasons.clone();
        unique.sort_unstable();
        unique.dedup();
        return Some(format!("Aucune image : le portail répond par une erreur ({}) sur ces chaînes — le fournisseur {label} ne les sert pas en ce moment (ses autres chaînes peuvent marcher). Dis-le tel quel.", unique.join(", ")));
    }
    Some("Aucune chaîne n'a renvoyé d'image. Elles sont peut-être réellement éteintes, MAIS le portail a peut-être aussi refusé les connexions (limite de connexions ?) : ne conclus pas qu'elles sont mortes, dis que le scan n'a rien capté et propose de réessayer plus tard.".into())
}

pub fn build(job: &ScanJob, scan: &Scan, reference: Option<&Reference>, usable: bool, follow: &Follow) -> Value {
    let mut out = json!({ "scan": "fait", "fournisseur": provider_fr(&job.provider), "query": if job.query.is_empty() { Value::Null } else { json!(job.query) }, "scan_n": scan.id, "groupes": scan.groups.iter().map(|g| group_json(scan, g)).collect::<Vec<_>>() });
    if job.compare {
        out["reference"] = match reference.filter(|_| usable) {
            Some(r) => json!({ "lecture": matching::resume_fr(Some(&r.reading)), "source": r.source, "age_s": (scan.at - r.at).max(0) / 1000 }),
            None => json!({ "lecture": matching::resume_fr(reference.map(|r| &r.reading)), "note": "image du direct illisible ou sans match : précise le match recherché avec cherche='<équipe A> - <équipe B>'" }),
        };
    }
    if let Some(blind) = blind(scan) {
        out["sans_image"] = blind;
    }
    if follow.vision_errors > 0 {
        out["vision_erreurs"] = json!(follow.vision_errors);
    }
    if follow.frames_unreadable {
        out["lecture_image"] = json!("indisponible : le module live ne donne pas accès aux images du scan (méthode scanFrame absente) ; seul l'état de chaque capture (noire, sans réponse) est connu, aucun contenu ni verdict");
    }
    if follow.late {
        out["delai_depasse"] = json!("scan écourté (trop long) : certaines chaînes n'ont pas été testées ou analysées");
    }
    if let Some(text) = warning(scan, &job.provider) {
        out["avertissement"] = json!(text);
    }
    let any_direct = scan.groups.iter().any(|g| g.kind == "direct");
    if !any_direct && !scan.groups.is_empty() && !follow.frames_unreadable {
        out["aucun_direct"] = json!("Aucune chaîne ne montre d'horloge ni de score : probablement AUCUN match en direct sur ces chaînes (souvent une bande promo en boucle).");
    }
    if scan.groups.iter().any(|g| g.verdict == Some(Verdict::Yes)) {
        out["conseil"] = json!("une chaîne « oui » montre le même match que le direct : switch_provider(ref) si elle est chez l'autre fournisseur");
    }
    out["note"] = json!(NOTE);
    out
}
