use serde_json::{json, Map, Value};

pub const MAX_STR: usize = 80;
pub const MAX_TITLE: usize = 60;
pub const MAX_UNIT: usize = 12;
pub const TABLE_COLS: (usize, usize) = (1, 6);
pub const TABLE_ROWS: (usize, usize) = (1, 12);
pub const CHART_POINTS: (usize, usize) = (2, 24);
pub const START_MIN: f64 = 946_684_800.0;
pub const START_MAX: f64 = 4_102_444_800.0;
const OUTCOMES: [&str; 3] = ["win", "draw", "loss"];
const MOVE_MAX: i64 = 20;

fn text(value: Option<&Value>, limit: usize) -> Option<String> {
    let s = value?.as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= limit).then(|| s.to_string())
}

fn finite(value: &Value) -> bool {
    value.is_number() && value.as_f64().is_some_and(f64::is_finite)
}

fn cell(value: &Value) -> Option<Value> {
    match value {
        Value::String(s) if s.chars().count() <= MAX_STR => Some(value.clone()),
        v if finite(v) => Some(v.clone()),
        _ => None,
    }
}

fn index(value: Option<&Value>, below: usize) -> Option<Option<usize>> {
    match value {
        None | Some(Value::Null) => Some(None),
        Some(v) => v
            .as_u64()
            .map(|n| n as usize)
            .filter(|n| *n < below)
            .map(Some),
    }
}

fn live(value: &Value, rows: usize, cols: usize) -> Option<Value> {
    let list = value.get("rows")?.as_array().filter(|l| l.len() == rows)?;
    let mut out = Vec::new();
    for row in list {
        let flag = row.get("live").map_or(Some(false), Value::as_bool)?;
        let outcome = match row.get("outcome") {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if OUTCOMES.contains(&s.as_str()) => Some(s.clone()),
            _ => return None,
        };
        let step = row
            .get("move")
            .map_or(Some(0), |m| m.as_i64().filter(|_| !m.is_boolean()))?;
        if step.abs() > MOVE_MAX {
            return None;
        }
        out.push(json!({"live": flag, "outcome": outcome, "move": step}));
    }
    let mut result = Map::from_iter([("rows".to_string(), Value::Array(out))]);
    for key in ["points_col", "match_col"] {
        if let Some(i) = index(value.get(key), cols)? {
            result.insert(key.into(), json!(i));
        }
    }
    Some(Value::Object(result))
}

fn titled(card: &Value, out: &mut Map<String, Value>) -> Option<()> {
    if card.get("title").is_some_and(|t| !t.is_null()) {
        out.insert("title".into(), json!(text(card.get("title"), MAX_TITLE)?));
    }
    Some(())
}

fn table(card: &Value) -> Option<Value> {
    let columns = card.get("columns")?.as_array()?;
    let rows = card.get("rows")?.as_array()?;
    if !(TABLE_COLS.0..=TABLE_COLS.1).contains(&columns.len())
        || !(TABLE_ROWS.0..=TABLE_ROWS.1).contains(&rows.len())
    {
        return None;
    }
    let names: Option<Vec<String>> = columns.iter().map(|c| text(Some(c), MAX_STR)).collect();
    let names = names?;
    let lines: Option<Vec<Vec<Value>>> = rows
        .iter()
        .map(|r| {
            r.as_array()
                .filter(|r| r.len() == names.len())
                .and_then(|r| r.iter().map(cell).collect())
        })
        .collect();
    let lines = lines?;
    let mut out = Map::from_iter([
        ("kind".to_string(), json!("table")),
        ("columns".to_string(), json!(names)),
        ("rows".to_string(), json!(lines)),
    ]);
    if let Some(l) = card.get("live").filter(|l| !l.is_null()) {
        out.insert("live".into(), live(l, lines.len(), names.len())?);
    }
    titled(card, &mut out)?;
    if card.get("note").is_some_and(|n| !n.is_null()) {
        out.insert("note".into(), json!(text(card.get("note"), MAX_STR)?));
    }
    Some(Value::Object(out))
}

fn chart(card: &Value) -> Option<Value> {
    let style = card
        .get("style")?
        .as_str()
        .filter(|s| ["bar", "line"].contains(s))?;
    let points = card
        .get("points")?
        .as_array()
        .filter(|p| (CHART_POINTS.0..=CHART_POINTS.1).contains(&p.len()))?;
    let cleaned: Option<Vec<Value>> = points
        .iter()
        .map(|p| {
            let label = text(p.get("label"), MAX_STR)?;
            let value = p.get("value").filter(|v| finite(v))?.clone();
            Some(json!({"label": label, "value": value}))
        })
        .collect();
    let mut out = Map::from_iter([
        ("kind".to_string(), json!("chart")),
        ("style".to_string(), json!(style)),
        ("points".to_string(), json!(cleaned?)),
    ]);
    titled(card, &mut out)?;
    if card.get("unit").is_some_and(|u| !u.is_null()) {
        out.insert("unit".into(), json!(text(card.get("unit"), MAX_UNIT)?));
    }
    Some(Value::Object(out))
}

fn match_card(card: &Value) -> Option<Value> {
    let mut out = Map::from_iter([
        ("kind".to_string(), json!("match")),
        ("home".to_string(), json!(text(card.get("home"), MAX_STR)?)),
        ("away".to_string(), json!(text(card.get("away"), MAX_STR)?)),
    ]);
    for key in ["competition", "score", "channel"] {
        if card.get(key).is_some_and(|v| !v.is_null()) {
            out.insert(key.into(), json!(text(card.get(key), MAX_STR)?));
        }
    }
    if let Some(start) = card.get("start").filter(|s| !s.is_null()) {
        let seconds = start
            .as_f64()
            .filter(|_| finite(start))
            .filter(|s| (START_MIN..=START_MAX).contains(s))?;
        out.insert("start".into(), json!(seconds as i64));
    }
    if let Some(status) = card.get("status").filter(|s| !s.is_null()) {
        out.insert(
            "status".into(),
            json!(status
                .as_str()
                .filter(|s| ["upcoming", "live", "ended"].contains(s))?),
        );
    }
    if let Some(action) = card.get("action").filter(|a| !a.is_null()) {
        let ok = action.get("id").and_then(Value::as_str) == Some("watch")
            && out.contains_key("channel");
        let label = text(action.get("label"), MAX_STR).filter(|_| ok)?;
        out.insert("action".into(), json!({"id": "watch", "label": label}));
    }
    Some(Value::Object(out))
}

pub fn validate(card: &Value) -> Option<Value> {
    match card.get("kind")?.as_str()? {
        "table" => table(card),
        "chart" => chart(card),
        "match" => match_card(card),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
