use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sport {
    #[serde(rename = "foot")]
    Soccer,
    Tennis,
    F1,
    Golf,
}

impl Sport {
    pub fn wire(self) -> &'static str {
        match self {
            Sport::Soccer => "foot",
            Sport::Tennis => "tennis",
            Sport::F1 => "f1",
            Sport::Golf => "golf",
        }
    }

    pub fn duration_s(self) -> f64 {
        match self {
            Sport::Soccer => 2.25 * 3600.0,
            Sport::Tennis => 3.0 * 3600.0,
            Sport::F1 => 2.5 * 3600.0,
            Sport::Golf => 10.0 * 3600.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Pre,
    In,
    Post,
}

impl State {
    pub fn parse(text: &str) -> Option<State> {
        match text {
            "pre" => Some(State::Pre),
            "in" => Some(State::In),
            "post" => Some(State::Post),
            _ => None,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            State::Pre => "pre",
            State::In => "in",
            State::Post => "post",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            State::In => 0,
            State::Pre => 1,
            State::Post => 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Line {
    #[serde(rename = "nom")]
    pub name: String,
    pub score: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SportEvent {
    pub id: String,
    pub espn: String,
    pub sport: Sport,
    pub code: String,
    pub competition: String,
    pub state: State,
    pub start: f64,
    pub end: Option<f64>,
    pub popularity: i32,
    pub lines: Vec<Line>,
    pub abbreviations: Vec<String>,
    pub clock: String,
    pub tournament: Option<String>,
}

impl SportEvent {
    pub fn expected_end(&self) -> f64 {
        self.end.unwrap_or(self.start + self.sport.duration_s())
    }

    pub fn is_ligue1(&self) -> bool {
        self.code == "fra.1"
    }

    pub fn team_names(&self) -> Option<(&str, &str)> {
        match self.lines.as_slice() {
            [home, away, ..] => Some((&home.name, &away.name)),
            _ => None,
        }
    }
}
