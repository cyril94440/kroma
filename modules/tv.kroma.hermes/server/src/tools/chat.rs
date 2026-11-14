use serde_json::{json, Value};

use super::{always, failure, text_arg, validate_card, Kind, Spec, ToolEnv};

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "reply",
            kind: Kind::Reply,
            activity: None,
            available: always,
            describe: || {
                (
                    "Poste une réponse dans le chat. Accompagnée d'outils d'enquête dans le même appel, c'est un message d'attente (« je regarde… »).",
                    json!({ "type": "object", "properties": { "message": { "type": "string", "description": "Réponse courte, en français, Markdown léger" } }, "required": ["message"] }),
                )
            },
            handler: reply,
        },
        Spec {
            name: "show_card",
            kind: Kind::Reply,
            activity: Some("card"),
            available: always,
            describe: || {
                (
                    "Poste un message avec une carte (tableau, graphique ou fiche de match) bâtie à partir de données trouvées par tes outils. `text` est la phrase de repli, toujours affichée. Jamais de données inventées.",
                    json!({
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" },
                            "card": {
                                "type": "object",
                                "properties": {
                                    "kind": { "type": "string", "enum": ["table", "chart", "match"] },
                                    "title": { "type": "string" },
                                    "columns": { "type": "array", "items": { "type": "string" } },
                                    "rows": { "type": "array", "items": { "type": "array", "items": {} } },
                                    "note": { "type": "string" },
                                    "style": { "type": "string", "enum": ["bar", "line"] },
                                    "unit": { "type": "string" },
                                    "points": { "type": "array", "items": { "type": "object", "properties": { "label": { "type": "string" }, "value": { "type": "number" } } } },
                                    "home": { "type": "string" },
                                    "away": { "type": "string" },
                                    "competition": { "type": "string" },
                                    "score": { "type": "string" },
                                    "start": { "type": "integer", "description": "secondes epoch" },
                                    "status": { "type": "string", "enum": ["upcoming", "live", "ended"] },
                                    "channel": { "type": "string" },
                                    "action": { "type": "object", "properties": { "id": { "type": "string", "enum": ["watch"] }, "label": { "type": "string" } } },
                                },
                                "required": ["kind"],
                            },
                        },
                        "required": ["text", "card"],
                    }),
                )
            },
            handler: show_card,
        },
        Spec {
            name: "ignore",
            kind: Kind::Ignore,
            activity: None,
            available: always,
            describe: || {
                (
                    "Ne rien poster : le message n'était pas adressé à toi (discussion entre spectateurs).",
                    json!({ "type": "object", "properties": { "reason": { "type": "string" } } }),
                )
            },
            handler: ignore,
        },
    ]
}

fn post(env: &mut ToolEnv, text: &str, card: Option<Value>) -> Result<(), String> {
    env.deps.room.say(text, card, Some(&env.message.id)).map_err(|e| format!("{e:#}"))?;
    if env.ctx.reply_is_progress {
        env.ctx.progress_posted = true;
    } else {
        env.ctx.answered = true;
    }
    Ok(())
}

fn reply(env: &mut ToolEnv, args: &Value) -> Value {
    let message = args.get("message").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if message.is_empty() {
        return json!("ok");
    }
    match post(env, &message, None) {
        Ok(()) => json!("ok"),
        Err(e) => failure(&e),
    }
}

fn show_card(env: &mut ToolEnv, args: &Value) -> Value {
    let message = args.get("text").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if message.is_empty() {
        return json!({ "erreur": "text est obligatoire (phrase de repli, toujours affichée)" });
    }
    let card = args.get("card").and_then(validate_card);
    let invalid = card.is_none();
    if let Err(e) = post(env, &message, card) {
        return failure(&e);
    }
    if invalid {
        env.deps.audit("decisions", "card_rejected", json!({ "kind": args.pointer("/card/kind") }));
        return json!({ "status": "carte_invalide", "note": "carte retirée (catalogue ou bornes non respectés) ; le texte a bien été posté seul" });
    }
    json!("ok")
}

fn ignore(env: &mut ToolEnv, args: &Value) -> Value {
    env.deps.audit("decisions", "ignore", json!({ "user": env.message.user_id, "reason": text_arg(args, "reason") }));
    json!("ignored")
}
