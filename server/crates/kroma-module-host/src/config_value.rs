use serde_json::Value;

pub(crate) fn as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub(crate) fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn as_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => Some(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_number_reads_as_an_integer_whether_it_is_stored_as_number_or_text() {
        assert_eq!(as_i64(&json!(7)), Some(7));
        assert_eq!(as_i64(&json!("50")), Some(50));
        assert_eq!(as_i64(&json!(2.0)), Some(2));
        assert_eq!(as_i64(&json!("many")), None);
        assert_eq!(as_i64(&Value::Null), None);
    }

    #[test]
    fn a_flag_reads_as_a_bool_whether_it_is_stored_as_bool_or_text() {
        assert_eq!(as_bool(&json!(true)), Some(true));
        assert_eq!(as_bool(&json!("false")), Some(false));
        assert_eq!(as_bool(&json!("yes")), None);
    }

    #[test]
    fn null_is_no_text_and_anything_else_is_its_json() {
        assert_eq!(as_string(&Value::Null), None);
        assert_eq!(as_string(&json!("a")), Some("a".into()));
        assert_eq!(as_string(&json!(3)), Some("3".into()));
    }
}
