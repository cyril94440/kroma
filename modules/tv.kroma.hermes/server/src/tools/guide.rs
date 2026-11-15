use serde_json::{json, Value};

use super::live::request_channel;
use super::{always, failure, int_arg, text_arg, Kind, Spec, ToolEnv};
use crate::paris::{clock_label, long_label};
use crate::peers::Programme;
use crate::strings;
use crate::webcache::{ambiguous, exact_title_first, is_foreign, normalise, Entry};

const HORIZON_MS: i64 = 14 * 3_600_000;
const STOP: &[&str] = &[
    "le", "la", "les", "l", "un", "une", "des", "de", "du", "d", "au", "aux", "a", "et", "ou", "en", "sur", "dans", "avec", "pour", "que", "qui", "ce", "cet", "cette", "ces", "ca",
    "il", "elle", "on", "y", "est", "sont", "moi", "nous", "soir", "matin", "midi", "aujourd", "hui", "demain", "maintenant", "match", "matchs", "programme", "programmes",
    "emission", "emissions", "mets", "mettre", "affiche", "afficher", "regarde", "regarder", "voir", "chaine", "chaines", "direct", "live", "tv", "television",
];

pub fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "search_guide",
            kind: Kind::Investigation,
            activity: Some("guide"),
            available: always,
            describe: || {
                (
                    "Consulte le GUIDE TV en lecture seule (aucun effet sur la diffusion) : programmes en cours et à venir (~14 h) correspondant aux mots-clés, avec description, horaires et chaîne. Pour toute QUESTION sur les programmes.",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string", "description": "mots-clés courts" },
                        "limit": { "type": "integer", "description": "défaut 25, max 40" },
                    }, "required": ["query"] }),
                )
            },
            handler: search_guide,
        },
        Spec {
            name: "find_program",
            kind: Kind::Action,
            activity: Some("guide"),
            available: always,
            describe: || {
                (
                    "Cherche un programme ou un événement dans le guide (« PSG Liverpool ») et met la chaîne qui le diffuse, avec le vote habituel. Change l'image POUR TOUT LE MONDE : seulement si on demande de le METTRE, jamais pour une question. query = mots-clés courts (équipes, compétition, titre).",
                    json!({ "type": "object", "properties": {
                        "query": { "type": "string" },
                        "etrangere_ok": { "type": "boolean", "description": "accepte une chaîne étrangère, seulement après avoir cherché une diffusion française" },
                    }, "required": ["query"] }),
                )
            },
            handler: find_program,
        },
    ]
}

pub fn tokens(query: &str) -> Vec<String> {
    let normalised = normalise(query);
    let raw: Vec<String> = normalised.split(' ').filter(|w| w.len() > 1).map(str::to_string).collect();
    let kept: Vec<String> = raw.iter().filter(|w| !STOP.contains(&w.as_str())).cloned().collect();
    if kept.is_empty() { raw } else { kept }
}

fn programme_json(p: &Programme, now: i64) -> Value {
    json!({
        "titre": p.title,
        "description": p.description,
        "chaine": p.channel_name,
        "ref": p.channel_id,
        "debut": clock_label(p.start),
        "fin": clock_label(p.end),
        "en_direct": p.start <= now && now < p.end,
    })
}

fn search_guide(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    let limit = int_arg(args, "limit", 25).clamp(1, 40) as usize;
    let now = env.deps.now();
    let found = match env.deps.control.epg(Some(&query), Some(now), Some(now + HORIZON_MS), 300) {
        Ok(found) => found,
        Err(e) => return failure(&format!("guide indisponible : {e:#}")),
    };
    let mut seen: Vec<(String, i64)> = Vec::new();
    let mut kept: Vec<&Programme> = Vec::new();
    for p in found.iter().filter(|p| p.end > now) {
        let key = (p.title.clone(), p.start);
        if !seen.contains(&key) {
            seen.push(key);
            kept.push(p);
        }
    }
    kept.sort_by_key(|p| (!(p.start <= now), p.start));
    kept.truncate(limit);
    env.deps.audit("sources", "search_guide", json!({ "query": query, "raw": found.len(), "kept": kept.len() }));
    json!({ "heure_actuelle": long_label(now), "programmes": kept.iter().map(|p| programme_json(p, now)).collect::<Vec<_>>() })
}

struct Hit {
    entry: Entry,
    programme: Programme,
}

fn collect(env: &mut ToolEnv, query: &str, full_only: bool) -> Result<(Vec<Hit>, Vec<Hit>), String> {
    let now = env.deps.now();
    let wanted = tokens(query);
    let found = env.deps.control.epg(Some(query), Some(now), Some(now + HORIZON_MS), 60).map_err(|e| format!("{e:#}"))?;
    let mut ranks: Vec<(String, f64)> = Vec::new();
    let (mut live, mut upcoming) = (Vec::new(), Vec::new());
    for p in found.into_iter().filter(|p| p.end > now) {
        let haystack = normalise(&format!("{} {}", p.title, p.description));
        let matched = wanted.iter().filter(|t| haystack.split(' ').any(|w| w == t.as_str())).count();
        if matched == 0 || (full_only && matched < wanted.len()) {
            continue;
        }
        let rank = match ranks.iter().find(|(id, _)| *id == p.channel_id) {
            Some((_, r)) => *r,
            None => {
                let r = env.deps.control.resolve(&p.channel_name, 5, None).ok().and_then(|c| c.into_iter().find(|c| c.id == p.channel_id)).map_or(0.0, |c| c.score);
                ranks.push((p.channel_id.clone(), r));
                r
            }
        };
        let entry = Entry {
            rank,
            title: p.title.clone(),
            matched,
            channel_id: p.channel_id.clone(),
            channel_name: p.channel_name.clone(),
            start: p.start,
            end: p.end,
            domestic: !is_foreign(&p.channel_name),
        };
        let hit = Hit { entry, programme: p };
        if hit.programme.start <= now { live.push(hit) } else { upcoming.push(hit) }
    }
    Ok((live, upcoming))
}

fn hints(env: &mut ToolEnv, query: &str) -> Vec<String> {
    if !env.model.can_search() {
        return Vec::new();
    }
    let now = env.deps.now();
    if let Some(cached) = env.deps.hint_cache.get(query, now) {
        return cached;
    }
    let ask = format!(
        "Nous sommes le {} (heure de Paris). Un téléspectateur demande à voir à la TV : « {query} ». Fais une recherche web si nécessaire pour identifier précisément l'événement ou le programme visé. \
         Réponds UNIQUEMENT par 2 à 4 mots-clés courts séparés par des virgules, utilisables tels quels dans un guide TV français (noms d'équipes, compétition, titre).",
        long_label(now)
    );
    let words: Vec<String> = match env.model.web_search(&ask) {
        Ok(answer) => answer.text.split([',', '\n', ';']).map(|k| k.trim().to_string()).filter(|k| k.chars().count() > 1 && k.chars().count() < 40).take(4).collect(),
        Err(e) => {
            env.deps.audit("sources", "web_hint_error", json!({ "query": query, "error": e.to_string() }));
            Vec::new()
        }
    };
    env.deps.audit("sources", "web_hint", json!({ "query": query, "keywords": words }));
    if !words.is_empty() {
        env.deps.hint_cache.put(query, words.clone(), now);
    }
    words
}

fn suggestion(h: &Hit) -> Value {
    json!({ "programme": h.programme.title, "chaine": h.programme.channel_name, "ref": h.programme.channel_id, "debut": clock_label(h.programme.start) })
}

fn find_program(env: &mut ToolEnv, args: &Value) -> Value {
    let query = text_arg(args, "query");
    if query.is_empty() {
        return failure("query est obligatoire");
    }
    let foreign_ok = args.get("etrangere_ok").and_then(Value::as_bool).unwrap_or(false);
    let (mut live, mut upcoming) = match collect(env, &query, true) {
        Ok(found) => found,
        Err(e) => return json!({ "ok": false, "status": "no_epg", "erreur": format!("guide indisponible : {e}") }),
    };
    let mut via_web = None;
    if live.is_empty() && upcoming.is_empty() {
        let found = hints(env, &query);
        for hint in &found {
            if let Ok((l, u)) = collect(env, hint, true) {
                for (list, extra) in [(&mut live, l), (&mut upcoming, u)] {
                    for hit in extra {
                        if !list.iter().any(|x| x.programme.channel_id == hit.programme.channel_id && x.programme.start == hit.programme.start) {
                            list.push(hit);
                        }
                    }
                }
            }
        }
        if (!live.is_empty() || !upcoming.is_empty()) && !found.is_empty() {
            via_web = Some(found.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
        }
    }
    env.deps.audit("sources", "find_program", json!({ "query": query, "live": live.len(), "upcoming": upcoming.len(), "web": via_web }));
    if !foreign_ok && !live.iter().chain(upcoming.iter()).any(|h| h.entry.domestic) && (!live.is_empty() || !upcoming.is_empty()) {
        let best = live.first().or(upcoming.first()).map(|h| (h.programme.title.clone(), h.programme.channel_name.clone(), h.programme.start));
        let (title, channel, start) = best.unwrap_or_default();
        env.deps.audit("decisions", "find_program_foreign_only", json!({ "query": query, "channel": channel, "programme": title }));
        return json!({
            "ok": false, "status": "seulement_etranger", "programme": title, "chaine_etrangere": channel, "debut": clock_label(start),
            "consigne": "Le guide ne trouve ce programme QUE sur une chaîne étrangère. Règle de la famille : diffuseur FRANÇAIS d'abord. Cherche-le en France (web_search « <programme> diffusion TV France », puis search_guide avec les noms français, ou benchmark_channels(query=<famille>, cherche='<A> - <B>') pour un multiplex sans guide) ; seulement s'il n'existe aucune diffusion française : find_program(query, etrangere_ok=true), en disant que c'est une chaîne étrangère.",
        });
    }
    if !live.is_empty() {
        let mut entries: Vec<Entry> = live.iter().map(|h| h.entry.clone()).collect();
        exact_title_first(&mut entries, &query);
        let ambiguity: Vec<Entry> = ambiguous(&entries, &query, |e| if e.domestic { 1.0 } else { 0.0 }).into_iter().cloned().collect();
        if !ambiguity.is_empty() {
            env.deps.audit("decisions", "find_program_ambiguous", json!({ "query": query, "candidates": ambiguity.iter().map(|e| e.title.clone()).collect::<Vec<_>>() }));
            let suggestions: Vec<Value> = ambiguity
                .iter()
                .take(4)
                .filter_map(|e| live.iter().find(|h| h.entry.title == e.title && h.entry.channel_id == e.channel_id).map(suggestion))
                .collect();
            return json!({ "ok": false, "status": "ambiguous", "suggestions": suggestions });
        }
        let best = &entries[0];
        let (id, name, title) = (best.channel_id.clone(), best.channel_name.clone(), best.title.clone());
        let note = via_web.map_or_else(|| title.clone(), |w| format!("{title} (trouvé via le web : {w})"));
        let mut out = request_channel(env, &id, &name, &note, false);
        out["program"] = json!(title);
        return out;
    }
    if !upcoming.is_empty() {
        upcoming.sort_by(|a, b| b.entry.rank.total_cmp(&a.entry.rank).then(a.entry.start.cmp(&b.entry.start)));
        let first = &upcoming[0];
        let extra = via_web.map(|w| format!(" (identifié via le web : {w})")).unwrap_or_default();
        let lang = env.deps.settings().language;
        let text = strings::text(&lang, "hermes.upcoming", &[("name", &env.message.name), ("title", &first.programme.title), ("time", &clock_label(first.programme.start)), ("channel", &first.programme.channel_name), ("extra", &extra)]);
        let out = json!({ "status": "upcoming", "program": first.programme.title, "channel": first.programme.channel_name, "start": first.programme.start });
        return match env.deps.room.say(&text, None, Some(&env.message.id)) {
            Ok(_) => {
                env.ctx.answered = true;
                out
            }
            Err(e) => failure(&format!("{e:#}")),
        };
    }
    let (pl, pu) = collect(env, &query, false).unwrap_or_default();
    let mut partials: Vec<Hit> = pl.into_iter().chain(pu).collect();
    let now = env.deps.now();
    partials.sort_by(|a, b| (a.programme.start > now).cmp(&(b.programme.start > now)).then(b.entry.rank.total_cmp(&a.entry.rank)));
    partials.truncate(4);
    if !partials.is_empty() {
        return json!({ "ok": false, "status": "partial_matches", "suggestions": partials.iter().map(suggestion).collect::<Vec<_>>() });
    }
    json!({
        "ok": false, "status": "not_found",
        "suite_possible": "un multiplex (Ligue1+, DAZN…) n'a pas de guide : web_search pour savoir où passe le match, puis benchmark_channels(query=<famille>, cherche='<équipe A> - <équipe B>') pour le retrouver à l'image (ou list_channels pour la chaîne)",
    })
}
