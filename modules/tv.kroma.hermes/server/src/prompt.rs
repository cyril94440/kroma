use crate::paris::long_label;
use crate::peers::{RoomMessage, Status};
use crate::tools::ToolSet;

mod sections;

pub use sections::SECTIONS;

pub const RULES_MARK: &str = "=== RÈGLES MAISON ===";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenVote {
    pub label: String,
    pub requester: String,
    pub waiting_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenPoll {
    pub question: String,
    pub kind: String,
    pub options: Vec<String>,
}

pub struct Context<'a> {
    pub now: i64,
    pub author: &'a str,
    pub text: &'a str,
    pub status: Option<&'a Status>,
    pub vote: Option<&'a OpenVote>,
    pub poll: Option<&'a OpenPoll>,
    pub history: &'a [RoomMessage],
    pub notes: &'a [String],
    pub agent_name: &'a str,
}

pub struct SystemPrompt {
    pub text: String,
    pub sections: Vec<&'static str>,
}

/// The system instruction for the tools that exist this turn: a section is in
/// only when a tool it talks about is declared, and the house rules close it.
pub fn system_instruction(tools: &ToolSet, rules: &str, language: &str, has_external: bool) -> SystemPrompt {
    let kept: Vec<&sections::Section> = SECTIONS
        .iter()
        .filter(|s| s.needs.is_empty() || s.needs.iter().any(|n| tools.has(n) || (*n == "__external__" && has_external)))
        .collect();
    let mut text = kept.iter().map(|s| s.text).collect::<Vec<_>>().join("\n\n");
    let answer_in = if language == "en" { "anglais" } else { "français" };
    text.push_str(&format!("\n\nLangue de tes réponses : {answer_in}.\n\n{RULES_MARK}\n{rules}"));
    SystemPrompt { text, sections: kept.iter().map(|s| s.id).collect() }
}

fn status_lines(status: &Status, lines: &mut Vec<String>) {
    if let Some(channel) = &status.channel {
        let programme = status.now.as_ref().filter(|n| !n.title.is_empty()).map(|n| format!(" — en ce moment : {}", n.title)).unwrap_or_default();
        lines.push(format!("[état] Chaîne en cours : {} (réf {}){programme}", channel.name, channel.id));
    } else {
        lines.push("[état] Aucune chaîne en cours.".into());
    }
    if status.state == "offAir" {
        lines.push("[état] La chaîne en cours est HORS ANTENNE (elle ne diffuse rien).".into());
    }
    if status.health.black {
        lines.push("[état] Image du direct : NOIRE (la chaîne ne diffuse rien en ce moment).".into());
    }
    if status.health.stutter || status.health.frozen {
        lines.push("[état] Flux : saccades ou image figée côté fournisseur.".into());
    }
    if !status.provider.is_empty() {
        lines.push(format!("[état] Fournisseur IPTV actif : {}", status.provider));
    }
    if !status.viewers.is_empty() {
        lines.push(format!("[état] Spectateurs : {}", status.viewers.iter().map(|v| v.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
}

/// The preamble of one message: date, what is on, open vote and poll, recent
/// chat, what is known of the author, then the message itself.
pub fn context(input: &Context) -> String {
    let mut lines = vec![format!("[état] Nous sommes le {} (Paris).", long_label(input.now))];
    if let Some(poll) = input.poll {
        let options = if poll.options.is_empty() { String::new() } else { format!(" — options : {}", poll.options.join(", ")) };
        lines.push(format!(
            "[état] SONDAGE EN COURS : « {} » (type {}{options}). Si le nouveau message est une réponse au sondage, enregistre-la avec record_poll_vote(value=valeur) ; si c'est une discussion sans rapport, ignore.",
            poll.question, poll.kind
        ));
    }
    if let Some(status) = input.status {
        status_lines(status, &mut lines);
    }
    if let Some(vote) = input.vote {
        let waiting = if vote.waiting_on.is_empty() { "personne".to_string() } else { vote.waiting_on.join(", ") };
        lines.push(format!("[état] VOTE EN COURS : {} (demandé par {}) — en attente de : {waiting}.", vote.label, vote.requester));
    }
    if !input.notes.is_empty() {
        lines.push(format!("[ce que tu sais de {}]", input.author));
        lines.extend(input.notes.iter().map(|n| format!("- {n}")));
    }
    if !input.history.is_empty() {
        lines.push("[historique récent du chat]".into());
        for m in input.history {
            let who = if m.kind == "agent" { input.agent_name } else if m.name.is_empty() { "?" } else { &m.name };
            lines.push(format!("{who}: {}", m.text));
        }
    }
    lines.push("[nouveau message à traiter]".into());
    lines.push(format!("{}: {}", input.author, input.text));
    lines.join("\n")
}

#[cfg(test)]
mod tests;
