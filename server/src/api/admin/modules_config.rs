use std::collections::BTreeMap;

use kroma_module_manifest::ConfigField;
use serde_json::{Map, Value};

pub(super) fn config_view(fields: &[ConfigField], stored: &Map<String, Value>) -> BTreeMap<String, Value> {
    fields
        .iter()
        .map(|field| {
            let value = if field.secret {
                secret_marker(stored.get(&field.key))
            } else {
                stored.get(&field.key).cloned().unwrap_or_else(|| field.default.clone().map(Value::from).unwrap_or(Value::Null))
            };
            (field.key.clone(), value)
        })
        .collect()
}

fn secret_marker(stored: Option<&Value>) -> Value {
    match stored {
        Some(Value::String(text)) if !text.is_empty() => Value::Bool(true),
        Some(Value::Null) | Some(Value::String(_)) | None => Value::Null,
        Some(_) => Value::Bool(true),
    }
}

pub(super) fn writable_config(fields: &[ConfigField], values: BTreeMap<String, Value>) -> Map<String, Value> {
    values
        .into_iter()
        .filter(|(key, value)| {
            fields.iter().find(|field| &field.key == key).is_some_and(|field| !field.secret || !is_blank(value))
        })
        .collect()
}

fn is_blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn field(key: &str, secret: bool, default: Option<&str>) -> ConfigField {
        serde_json::from_value(json!({ "key": key, "label": key, "type": "string", "secret": secret, "default": default })).unwrap()
    }

    fn stored(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn a_secret_is_never_returned_only_whether_it_is_set() {
        let fields = [field("primaryUrl", true, None), field("backupUrl", true, None), field("agent", false, Some("VLC")), field("token", true, Some("preset"))];
        let stored = stored(json!({ "primaryUrl": "http://portal/get.php?password=hunter2", "backupUrl": "" }));

        let view = config_view(&fields, &stored);

        assert_eq!(serde_json::to_value(&view).unwrap(), json!({ "primaryUrl": true, "backupUrl": null, "agent": "VLC", "token": null }));
        assert!(!serde_json::to_string(&view).unwrap().contains("hunter2"));
    }

    #[test]
    fn a_blank_or_absent_secret_on_save_keeps_the_stored_one_and_a_new_one_replaces_it() {
        let fields = [field("primaryUrl", true, None), field("agent", false, None)];
        let body = |value: Value| value.as_object().unwrap().clone().into_iter().collect::<BTreeMap<_, _>>();

        assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": "", "agent": "" }))), stored(json!({ "agent": "" })));
        assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": null }))), Map::new());
        assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": "http://new", "other": 1 }))), stored(json!({ "primaryUrl": "http://new" })));
    }
}
