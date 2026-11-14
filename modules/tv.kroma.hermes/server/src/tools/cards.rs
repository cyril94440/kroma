use serde_json::{json, Map, Value};

const MAX_STR: usize = 80;
const MAX_TITLE: usize = 60;
const MAX_UNIT: usize = 12;
const START_MIN: i64 = 946_684_800;
const START_MAX: i64 = 4_102_444_800;
const OUTCOMES: [&str; 3] = ["win", "draw", "loss"];
const STATUSES: [&str; 3] = ["upcoming", "live", "ended"];
const STYLES: [&str; 2] = ["bar", "line"];

fn text(v: Option<&Value>, max: usize) -> Option<String> {
    let s = v?.as_str()?.trim();
    (!s.is_empty() && s.chars().count() <= max).then(|| s.to_string())
}

fn number(v: &Value) -> bool {
    v.is_number() && v.as_f64().is_some_and(f64::is_finite)
}

fn cell(v: &Value) -> Option<Value> {
    match v {
        Value::String(s) if s.chars().count() <= MAX_STR => Some(v.clone()),
        v if number(v) => Some(v.clone()),
        _ => None,
    }
}

fn optional_text(c: &Value, key: &str, max: usize) -> Result<Option<String>, ()> {
    match c.get(key) {
        None | Some(Value::Null) => Ok(None),
        some => text(some, max).map(Some).ok_or(()),
    }
}

fn live(v: &Value, rows: usize, cols: usize) -> Option<Value> {
    let list = v.get("rows")?.as_array().filter(|r| r.len() == rows)?;
    let mut out = Vec::new();
    for r in list {
        let flag = match r.get("live") {
            None => false,
            Some(Value::Bool(b)) => *b,
            _ => return None,
        };
        let outcome = r.get("outcome").filter(|o| !o.is_null());
        if outcome.is_some_and(|o| !o.as_str().is_some_and(|s| OUTCOMES.contains(&s))) {
            return None;
        }
        let movement = match r.get("move") {
            None => 0,
            Some(m) => m.as_i64().filter(|m| m.abs() <= 20)?,
        };
        out.push(json!({ "live": flag, "outcome": outcome.cloned().unwrap_or(Value::Null), "move": movement }));
    }
    let mut obj = Map::new();
    obj.insert("rows".into(), Value::Array(out));
    for key in ["points_col", "match_col"] {
        if let Some(x) = v.get(key).filter(|x| !x.is_null()) {
            let index = x.as_u64().filter(|i| (*i as usize) < cols)?;
            obj.insert(key.into(), json!(index));
        }
    }
    Some(Value::Object(obj))
}

fn table(c: &Value) -> Option<Value> {
    let cols = c.get("columns")?.as_array().filter(|c| (1..=6).contains(&c.len()))?;
    let rows = c.get("rows")?.as_array().filter(|r| (1..=12).contains(&r.len()))?;
    let columns: Vec<String> = cols.iter().map(|x| text(Some(x), MAX_STR)).collect::<Option<_>>()?;
    let mut lines = Vec::new();
    for r in rows {
        let cells = r.as_array().filter(|r| r.len() == columns.len())?;
        lines.push(Value::Array(cells.iter().map(cell).collect::<Option<_>>()?));
    }
    let mut out = json!({ "kind": "table", "columns": columns, "rows": lines });
    if let Some(l) = c.get("live").filter(|l| !l.is_null()) {
        out["live"] = live(l, lines_len(&out), columns.len())?;
    }
    if let Some(title) = optional_text(c, "title", MAX_TITLE).ok()? {
        out["title"] = json!(title);
    }
    if let Some(note) = optional_text(c, "note", MAX_STR).ok()? {
        out["note"] = json!(note);
    }
    Some(out)
}

fn lines_len(card: &Value) -> usize {
    card["rows"].as_array().map_or(0, Vec::len)
}

fn chart(c: &Value) -> Option<Value> {
    let style = c.get("style")?.as_str().filter(|s| STYLES.contains(s))?;
    let points = c.get("points")?.as_array().filter(|p| (2..=24).contains(&p.len()))?;
    let mut clean = Vec::new();
    for p in points {
        let label = text(p.get("label"), MAX_STR)?;
        let value = p.get("value").filter(|v| number(v))?;
        clean.push(json!({ "label": label, "value": value }));
    }
    let mut out = json!({ "kind": "chart", "style": style, "points": clean });
    if let Some(title) = optional_text(c, "title", MAX_TITLE).ok()? {
        out["title"] = json!(title);
    }
    if let Some(unit) = optional_text(c, "unit", MAX_UNIT).ok()? {
        out["unit"] = json!(unit);
    }
    Some(out)
}

fn fixture(c: &Value) -> Option<Value> {
    let mut out = json!({ "kind": "match", "home": text(c.get("home"), MAX_STR)?, "away": text(c.get("away"), MAX_STR)? });
    for key in ["competition", "score", "channel"] {
        if let Some(v) = optional_text(c, key, MAX_STR).ok()? {
            out[key] = json!(v);
        }
    }
    if let Some(start) = c.get("start").filter(|s| !s.is_null()) {
        let seconds = start.as_f64().filter(|s| s.is_finite() && (START_MIN as f64..=START_MAX as f64).contains(s))?;
        out["start"] = json!(seconds as i64);
    }
    if let Some(status) = c.get("status").filter(|s| !s.is_null()) {
        out["status"] = json!(status.as_str().filter(|s| STATUSES.contains(s))?);
    }
    if let Some(action) = c.get("action").filter(|a| !a.is_null()) {
        if action.get("id")? != "watch" || out.get("channel").is_none() {
            return None;
        }
        out["action"] = json!({ "id": "watch", "label": text(action.get("label"), MAX_STR)? });
    }
    Some(out)
}

/// A sanitised copy of a card from the closed catalogue (table, chart, match),
/// or `None`: an invalid card is dropped, never repaired, and the message that
/// carried it is posted as plain text.
pub fn validate_card(card: &Value) -> Option<Value> {
    match card.get("kind")?.as_str()? {
        "table" => table(card),
        "chart" => chart(card),
        "match" => fixture(card),
        _ => None,
    }
}
