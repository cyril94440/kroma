use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::Person;

pub const DEFAULT_SECONDS: i64 = 120;
pub const MAX_SECONDS: i64 = 900;
pub const MIN_SECONDS: i64 = 20;
pub const QUESTION_MAX: usize = 200;
pub const OPTION_MAX: usize = 60;
pub const OPTIONS_MAX: usize = 6;
pub const OPEN_ANSWER_MAX: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PollKind {
    Yesno,
    Choice,
    Score,
    Open,
}

impl PollKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "yesno" => Some(Self::Yesno),
            "choice" => Some(Self::Choice),
            "score" => Some(Self::Score),
            "open" => Some(Self::Open),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Poll {
    pub id: u64,
    pub question: String,
    pub kind: PollKind,
    pub options: Vec<String>,
    pub votes: BTreeMap<String, PollVote>,
    pub ends_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollVote {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    InvalidQuestionOrType,
    ChoiceNeedsTwoOptions,
}

impl PollError {
    pub fn code(&self) -> &'static str {
        match self {
            PollError::InvalidQuestionOrType => "invalidQuestionOrType",
            PollError::ChoiceNeedsTwoOptions => "choiceNeedsTwoOptions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollView {
    pub id: u64,
    pub question: String,
    pub kind: PollKind,
    pub options: Vec<String>,
    pub votes: BTreeMap<String, String>,
    pub by_answer: BTreeMap<String, Vec<String>>,
    pub total: usize,
    pub ends_at: i64,
    pub ends_in_s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_because: Option<String>,
}

fn fold_char(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' | 'á' => 'a',
        'ç' => 'c',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'î' | 'ï' | 'í' => 'i',
        'ô' | 'ö' | 'ó' => 'o',
        'ù' | 'û' | 'ü' | 'ú' => 'u',
        'ÿ' => 'y',
        'ñ' => 'n',
        other => other,
    }
}

pub fn fold(s: &str) -> String {
    s.trim().to_lowercase().chars().map(fold_char).collect()
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

fn score_shape(s: &str) -> Option<String> {
    let sep = s.find(['-', ':', '/'])?;
    let (a, b) = (s[..sep].trim(), s[sep + 1..].trim());
    let digits = |x: &str| (1..=2).contains(&x.len()) && x.chars().all(|c| c.is_ascii_digit());
    (digits(a) && digits(b)).then(|| format!("{a}-{b}"))
}

impl Poll {
    pub fn start(
        id: u64,
        question: &str,
        kind: &str,
        options: &[String],
        seconds: Option<i64>,
        now: i64,
    ) -> Result<Self, PollError> {
        let question = clip(question, QUESTION_MAX);
        let kind = PollKind::parse(kind).ok_or(PollError::InvalidQuestionOrType)?;
        if question.is_empty() {
            return Err(PollError::InvalidQuestionOrType);
        }
        let cleaned: Vec<String> = options
            .iter()
            .map(|o| clip(o, OPTION_MAX))
            .filter(|o| !o.is_empty())
            .take(OPTIONS_MAX)
            .collect();
        let options = match kind {
            PollKind::Yesno => vec!["Oui".to_string(), "Non".to_string()],
            PollKind::Choice if cleaned.len() < 2 => return Err(PollError::ChoiceNeedsTwoOptions),
            PollKind::Choice => cleaned,
            PollKind::Score | PollKind::Open => Vec::new(),
        };
        let seconds = seconds.unwrap_or(DEFAULT_SECONDS).clamp(MIN_SECONDS, MAX_SECONDS);
        Ok(Self {
            id,
            question,
            kind,
            options,
            votes: BTreeMap::new(),
            ends_at: now + seconds * 1000,
        })
    }

    pub fn normalize(&self, raw: &str) -> Option<String> {
        let v = clip(raw, OPTION_MAX);
        match self.kind {
            PollKind::Yesno => {
                let t = fold(&v);
                if t == "oui" || v == "\u{1F44D}" {
                    Some("Oui".into())
                } else if t == "non" || v == "\u{1F44E}" {
                    Some("Non".into())
                } else {
                    None
                }
            }
            PollKind::Choice => self.options.iter().find(|o| fold(o) == fold(&v)).cloned(),
            PollKind::Score => score_shape(&v),
            PollKind::Open => {
                if v.is_empty() || v.chars().count() > OPEN_ANSWER_MAX {
                    return None;
                }
                let same = self.votes.values().find(|x| fold(&x.value) == fold(&v));
                Some(same.map_or(v, |x| x.value.clone()))
            }
        }
    }

    pub fn vote(&mut self, voter: &Person, raw: &str) -> bool {
        let Some(value) = self.normalize(raw) else {
            return false;
        };
        self.votes.insert(
            voter.user_id.clone(),
            PollVote {
                name: voter.name.clone(),
                value,
            },
        );
        true
    }

    pub fn answer_from_chat(&self, text: &str) -> Option<String> {
        let t = text.to_lowercase();
        let t = t.trim();
        match self.kind {
            PollKind::Yesno if t == "oui" || t == "non" || text == "\u{1F44D}" || text == "\u{1F44E}" => {
                Some(text.into())
            }
            PollKind::Score if score_shape(t).is_some() && t.len() <= 5 => Some(t.into()),
            PollKind::Choice => Some(text.into()),
            _ => None,
        }
    }

    pub fn everyone_voted(&self, present: &[Person]) -> bool {
        let mut ids: Vec<&str> = present.iter().map(|p| p.user_id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len() >= 2 && ids.iter().all(|id| self.votes.contains_key(*id))
    }

    pub fn view(&self, now: i64) -> PollView {
        let mut by_answer: BTreeMap<String, Vec<String>> = BTreeMap::new();
        if matches!(self.kind, PollKind::Yesno | PollKind::Choice) {
            for o in &self.options {
                by_answer.entry(o.clone()).or_default();
            }
        }
        for v in self.votes.values() {
            by_answer.entry(v.value.clone()).or_default().push(v.name.clone());
        }
        PollView {
            id: self.id,
            question: self.question.clone(),
            kind: self.kind,
            options: self.options.clone(),
            votes: self.votes.iter().map(|(k, v)| (k.clone(), v.value.clone())).collect(),
            by_answer,
            total: self.votes.len(),
            ends_at: self.ends_at,
            ends_in_s: ((self.ends_at - now) / 1000).max(0),
            closed_because: None,
        }
    }

    pub fn summary(view: &PollView) -> String {
        if matches!(view.kind, PollKind::Score | PollKind::Open) {
            let parts: Vec<String> = view
                .by_answer
                .iter()
                .map(|(answer, who)| format!("{answer} ({})", who.join(", ")))
                .collect();
            return parts.join(" \u{b7} ");
        }
        let mut rows: Vec<(&String, &Vec<String>)> = view.by_answer.iter().collect();
        rows.sort_by_key(|(_, who)| std::cmp::Reverse(who.len()));
        rows.iter()
            .map(|(answer, who)| {
                if who.is_empty() {
                    format!("{answer} : 0")
                } else {
                    format!("{answer} : {} \u{2014} {}", who.len(), who.join(", "))
                }
            })
            .collect::<Vec<_>>()
            .join(" \u{b7} ")
    }
}

#[cfg(test)]
mod tests;
