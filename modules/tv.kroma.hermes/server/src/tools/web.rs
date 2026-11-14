use serde_json::{json, Value};

use super::{failure, text_arg, Availability, Kind, Spec, ToolEnv};
use crate::paris::{civil, long_label};
use crate::webcache::{has_date_mark, question_dated};

const ANSWER_MAX: usize = 1500;

pub fn specs() -> Vec<Spec> {
    vec![Spec {
        name: "web_search",
        kind: Kind::Investigation,
        activity: Some("web"),
        available: |a: &Availability| a.search,
        describe: || {
            (
                "Recherche web factuelle (horaires, droits TV, actualité, fait sur une personne réelle : seulement si une source le dit, sinon « je ne sais pas »). Mémoire de 10 min ; une question de score ou d'horaire sans repère daté dans la réponse est refusée (non_verifie).",
                json!({ "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }),
            )
        },
        handler: web_search,
    }]
}

fn web_search(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    let now = env.deps.now();
    let dated = question_dated(&query);
    if !dated {
        if let Some(cached) = env.deps.web_cache.get(&query, now) {
            env.deps.audit("sources", "web_search", json!({ "query": query, "cache": true }));
            return cached;
        }
    }
    let ask = format!(
        "Nous sommes le {} (heure de Paris). Réponds de façon factuelle, précise et concise (5 lignes maximum), en français, en précisant la date et l'heure de l'information (jamais de score ou d'horaire sans date) : {query}",
        long_label(now)
    );
    let answer = match env.model.web_search(&ask) {
        Ok(a) => a,
        Err(e) => {
            env.deps.audit("sources", "web_search_error", json!({ "query": query, "error": e.to_string() }));
            return failure(&e.to_string());
        }
    };
    let text: String = answer.text.chars().take(ANSWER_MAX).collect();
    env.deps.audit("sources", "web_search", json!({ "query": query, "answer": text, "sources": answer.sources }));
    if dated && !has_date_mark(&text, civil(now).year) {
        env.deps.audit("decisions", "web_search_unverified", json!({ "query": query }));
        return json!({ "non_verifie": true, "erreur": "réponse sans date ni heure : score/horaire non fiable, ne pas l'affirmer (reformuler la recherche ou le dire)" });
    }
    let mut out = json!({ "reponse": text, "consulte_le": crate::paris::dated_label(now) });
    if !answer.sources.is_empty() {
        out["sources"] = json!(answer.sources);
    }
    if !dated {
        env.deps.web_cache.put(&query, out.clone(), now);
    }
    out
}
