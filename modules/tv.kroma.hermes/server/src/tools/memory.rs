use serde_json::{json, Value};

use super::{always, failure, text_arg, Kind, Spec, ToolEnv};

pub const LESSONS_MAX: usize = 30;
pub const SUGGESTIONS_MAX: usize = 120;
pub const NOTES_MAX: usize = 12;
const DUPLICATE_WINDOW_MS: i64 = 30 * 60_000;

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "learn_lesson",
            kind: Kind::Effect,
            activity: None,
            available: always,
            describe: || {
                (
                    "Quand on te CORRIGE ou te donne une consigne durable : retiens une règle courte et générale (10 à 300 caractères). Elle rejoint les RÈGLES MAISON dès le message suivant. Puis reply (excuse brève + ce que tu retiens).",
                    json!({ "type": "object", "properties": { "lesson": { "type": "string" } }, "required": ["lesson"] }),
                )
            },
            handler: learn_lesson,
        },
        Spec {
            name: "save_suggestion",
            kind: Kind::Effect,
            activity: None,
            available: always,
            describe: || {
                (
                    "Mémoire durable des bugs et améliorations : dès que tu détectes un dysfonctionnement ou qu'une bonne idée émerge, enregistre-la SANS demander la permission ; l'administrateur relit la liste. Elle n'est pas active immédiatement : dis-le. Puis reply.",
                    json!({ "type": "object", "properties": {
                        "kind": { "type": "string", "enum": ["bug", "amelioration", "observation"] },
                        "titre": { "type": "string", "description": "résumé court (≤120 car.)" },
                        "detail": { "type": "string", "description": "contexte précis : qui, quand, symptômes (≤600 car.)" },
                    }, "required": ["kind", "titre", "detail"] }),
                )
            },
            handler: save_suggestion,
        },
        Spec {
            name: "note_person",
            kind: Kind::Effect,
            activity: None,
            available: always,
            describe: || {
                (
                    "Retient une préférence durable de la PERSONNE QUI T'ÉCRIT (équipe de cœur, chaîne favorite, genre aimé), 10 à 200 caractères. Jamais de note sur quelqu'un d'autre ; jamais de donnée sensible. Si elle demande d'oublier ce que tu sais d'elle : forget=true.",
                    json!({ "type": "object", "properties": {
                        "note": { "type": "string" },
                        "forget": { "type": "boolean" },
                    } }),
                )
            },
            handler: note_person,
        },
    ]
}

fn learn_lesson(env: &mut ToolEnv, args: &Value) -> Value {
    let lesson = env.deps.mask(&text_arg(args, "lesson"));
    if !(10..=300).contains(&lesson.chars().count()) {
        return failure("leçon trop courte ou trop longue (10-300 caractères)");
    }
    let now = env.deps.now();
    let dated = format!("[{}] {lesson}", crate::paris::date_label(now));
    env.deps.audit("decisions", "learn_lesson", json!({ "user": env.message.user_id, "lesson": lesson }));
    match env.deps.store.add_lesson(&dated, now, LESSONS_MAX) {
        Ok(total) => json!({ "ok": true, "lecons_total": total }),
        Err(e) => failure(&format!("{e:#}")),
    }
}

fn clip(text: String, max: usize) -> String {
    text.chars().take(max).collect()
}

fn save_suggestion(env: &mut ToolEnv, args: &Value) -> Value {
    let kind = match text_arg(args, "kind").to_lowercase().as_str() {
        k @ ("bug" | "amelioration") => k.to_string(),
        _ => "observation".to_string(),
    };
    let title = clip(env.deps.mask(&text_arg(args, "titre")), 120);
    let detail = clip(env.deps.mask(&text_arg(args, "detail")), 600);
    if title.chars().count() < 5 || detail.chars().count() < 10 {
        return failure("titre (≥5 car.) et detail (≥10 car.) requis");
    }
    let now = env.deps.now();
    if env.deps.seen_suggestion(&kind, &title, now, DUPLICATE_WINDOW_MS) {
        env.deps.audit("decisions", "save_suggestion", json!({ "kind": kind, "title": title, "duplicate": true }));
        return json!({ "ok": true, "deja_notee": true });
    }
    env.deps.audit("decisions", "save_suggestion", json!({ "kind": kind, "title": title, "by": env.message.user_id }));
    match env.deps.store.add_suggestion(&kind, &title, &detail, now, SUGGESTIONS_MAX) {
        Ok(total) => json!({ "ok": true, "total": total }),
        Err(e) => failure(&format!("{e:#}")),
    }
}

fn note_person(env: &mut ToolEnv, args: &Value) -> Value {
    let user = env.message.user_id.clone();
    if args.get("forget").and_then(Value::as_bool).unwrap_or(false) {
        env.deps.audit("decisions", "forget_person", json!({ "user": user }));
        return match env.deps.store.forget_person(&user) {
            Ok(removed) => json!({ "ok": true, "oubliees": removed }),
            Err(e) => failure(&format!("{e:#}")),
        };
    }
    let note = env.deps.mask(&text_arg(args, "note"));
    if !(10..=200).contains(&note.chars().count()) {
        return failure("note trop courte ou trop longue (10-200 caractères)");
    }
    env.deps.audit("decisions", "note_person", json!({ "user": user, "note": note }));
    match env.deps.store.add_note(&user, &env.message.name, &note, env.deps.now(), NOTES_MAX) {
        Ok(()) => json!({ "ok": true }),
        Err(e) => failure(&format!("{e:#}")),
    }
}
