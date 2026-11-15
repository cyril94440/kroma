use serde_json::{json, Value};

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    let mut parameters = json!({"type": "object", "properties": properties});
    if !required.is_empty() {
        parameters["required"] = json!(required);
    }
    json!({"name": name, "description": description, "parameters": parameters})
}

pub fn declared() -> Vec<Value> {
    vec![
        tool(
            "match_facts",
            "FAITS STRUCTURÉS d'un match de foot depuis ESPN (nous avons cette source, tu y as accès) : score, minute, statut (à venir / en cours / terminé), buteurs avec minute, cartons. OBLIGATOIRE pour tout score, résultat, « qui a marqué ? », minute ou carton d'un match (TOUTES compétitions : championnats, Ligue des champions, coupes, sélections), AVANT toute réponse et avant web_search. `equipes` = les équipes du match (« Lens - Lille », ou une seule : « PSG »). Sans `equipes`, renvoie la liste des matchs du jour. Les données sont volontairement décalées pour ne pas devancer l'image : ne cite rien de plus récent que ce que l'outil renvoie. Rien dans la réponse = tu ne sais pas : dis-le, n'invente JAMAIS un score, un buteur ou un match. Pour un classement : live_standings.",
            json!({
                "equipes": {"type": "string", "description": "ex: 'Lens - Lille' ou 'PSG'"},
                "competition": {"type": "string", "description": "facultatif, ex: 'Ligue 1' (sinon toutes les compétitions)"}
            }),
            &[],
        ),
        tool(
            "compos_match",
            "COMPOSITIONS RÉELLES d'un match (ESPN, toutes compétitions) : onze de départ par ligne avec la formation, banc, remplacements déjà visibles à l'image. OBLIGATOIRE pour « la compo », « l'équipe type », « qui joue ? », « qui est sur le banc ? », AVANT toute réponse et à la place de web_search. Sans `equipes` : le match de la chaîne du direct. Compos pas encore publiées (ESPN les donne ~1 h avant) : dis-le, n'en invente jamais.",
            json!({"equipes": {"type": "string", "description": "facultatif : ex. 'PSG' ou 'Villarreal - PSG' ; vide = le match du direct"}}),
            &[],
        ),
        tool(
            "live_standings",
            "Classement EN DIRECT / LIVE / PROVISOIRE d'un championnat (Ligue 1, Ligue 2, Premier League, Liga, Serie A, Bundesliga). Le CODE récupère lui-même le classement officiel et les scores des matchs EN COURS (source structurée), ajoute les points provisoires (3 victoire, 1 nul), recalcule la différence et trie. OBLIGATOIRE pour tout classement demandé « live », « en direct », « provisoire », et le meilleur choix pour un classement tout court : appelle-le avec `competition` SEUL (pas de web_search avant), puis show_card avec la `carte` rendue TELLE QUELLE. Ne fais jamais ce calcul toi-même, ne modifie aucun chiffre. `standings`/`matches` : seulement si l'outil répond que la source est indisponible et que tu as trouvé ces données ailleurs.",
            json!({
                "competition": {"type": "string", "description": "ex: 'Ligue 1'"},
                "standings": {"type": "array", "description": "classement officiel le plus récent", "items": {"type": "object", "properties": {"club": {"type": "string"}, "pts": {"type": "integer"}, "diff": {"type": "integer", "description": "différence de buts"}}, "required": ["club", "pts"]}},
                "matches": {"type": "array", "description": "matchs pas encore comptés dans standings (en cours)", "items": {"type": "object", "properties": {"home": {"type": "string"}, "away": {"type": "string"}, "score": {"type": "string", "description": "score actuel, ex: '2-1'"}, "minute": {"type": "string", "description": "ex: \"67'\""}}, "required": ["home", "away", "score"]}},
                "top": {"type": "integer", "description": "lignes à garder dans la carte (≤ 12, défaut 10)"}
            }),
            &["competition"],
        ),
        tool(
            "start_prono",
            "Lance un PRONO DE MATCH (score exact) à la demande (« fais un prono pour Le Havre – PSG »). Pour un match de foot précis d'une grande ligue : chaque spectateur saisit un score, clos au coup d'envoi, points 3 (score exact) / 1 (bon vainqueur) comptés par le CODE pendant la saison. Les pronos des matchs diffusés sur la chaîne du direct s'ouvrent déjà tout seuls : n'appelle cet outil que sur demande explicite. Réservé aux administrateurs (le code le vérifie lui-même). Le code vérifie le match (coup d'envoi proche ou au plus tard 10e minute) et peut refuser : rapporte son erreur telle quelle. Pour une autre question, un autre sport ou un sondage libre, utilise start_poll.",
            json!({
                "home": {"type": "string", "description": "Équipe qui reçoit, ex: 'Le Havre'"},
                "away": {"type": "string", "description": "Équipe qui se déplace, ex: 'PSG'"}
            }),
            &["home", "away"],
        ),
        tool(
            "sport_channel_hint",
            "Enregistre la chaîne qu'une recherche web (web_search) a donnée pour la diffusion d'un match du programme sport. Passe la réponse de la recherche TELLE QUELLE dans `answer` : le code lit lui-même le nom de la chaîne, tu n'as pas à le choisir. À appeler après web_search quand l'utilisateur demande sur quelle chaîne passe un match.",
            json!({
                "home": {"type": "string", "description": "Équipe qui reçoit"},
                "away": {"type": "string", "description": "Équipe qui se déplace"},
                "answer": {"type": "string", "description": "la réponse de la recherche web, telle quelle"}
            }),
            &["home", "away", "answer"],
        ),
        tool(
            "calibrate_clock",
            "Cale l'anti-spoiler sur l'image : donne le chrono du match tel qu'il est AFFICHÉ À L'ÉCRAN du direct (format mm:ss, ex. '52:13'), lu avec look_at_live. Le code vérifie que le chrono est plausible et l'utilise pour ne jamais annoncer un fait avant l'image. N'invente jamais un chrono : sans lecture claire, n'appelle pas cet outil.",
            json!({"clock": {"type": "string", "description": "chrono du match lu à l'écran, mm:ss"}}),
            &["clock"],
        ),
    ]
}
