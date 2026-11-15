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

pub(super) fn writable_config(fields: &[ConfigField], values: BTreeMap<String, Value>) -> Result<Map<String, Value>, String> {
    let mut written = Map::new();
    for (key, value) in values {
        let Some(field) = fields.iter().find(|field| field.key == key) else { continue };
        if !field.secret {
            written.insert(key, value);
            continue;
        }
        match value {
            Value::Null => {
                written.insert(key, Value::Null);
            }
            Value::String(text) if text.is_empty() => {}
            Value::String(_) => {
                written.insert(key, value);
            }
            _ if field.kind == "string" => return Err(key),
            _ => {
                written.insert(key, value);
            }
        }
    }
    Ok(written)
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

    fn body(value: Value) -> BTreeMap<String, Value> {
        value.as_object().unwrap().clone().into_iter().collect()
    }

    #[test]
    fn a_blank_or_absent_secret_on_save_keeps_the_stored_one_and_a_new_one_replaces_it() {
        let fields = [field("primaryUrl", true, None), field("agent", false, None)];

        assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": "", "agent": "" }))), Ok(stored(json!({ "agent": "" }))));
        assert_eq!(writable_config(&fields, body(json!({ "agent": "VLC" }))), Ok(stored(json!({ "agent": "VLC" }))));
        assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": "http://new", "other": 1 }))), Ok(stored(json!({ "primaryUrl": "http://new" }))));
    }

    #[test]
    fn an_explicit_null_clears_a_secret_and_never_touches_a_plain_field_it_does_not_name() {
        let fields = [field("primaryUrl", true, None), field("backupUrl", true, None), field("agent", false, None)];

        let written = writable_config(&fields, body(json!({ "primaryUrl": null, "agent": "VLC" }))).unwrap();

        assert_eq!(written, stored(json!({ "primaryUrl": null, "agent": "VLC" })));
        assert!(!written.contains_key("backupUrl"));
    }

    #[test]
    fn a_value_that_is_not_text_is_refused_for_a_string_secret_so_the_view_marker_is_never_stored() {
        let fields = [field("primaryUrl", true, None), field("agent", false, None)];

        for bad in [json!(true), json!(false), json!(3), json!({ "a": 1 }), json!(["x"])] {
            assert_eq!(writable_config(&fields, body(json!({ "primaryUrl": bad, "agent": "VLC" }))), Err("primaryUrl".to_string()));
        }
    }

    #[tokio::test]
    async fn the_config_route_stores_clears_and_refuses_a_secret_over_http() {
        use crate::api::test_support::{get, installed_module, send, test_app};

        let t = test_app();
        installed_module(&t, "tv.kroma.stub", r#", "config": [{ "key": "url", "label": "Url", "type": "string", "secret": true }]"#);
        let put = |body: Value| send(&t.app, "PUT", "/api/admin/modules/tv.kroma.stub/config", Some(&t.token), Some(body));
        let view = || async {
            let (_, listed) = get(&t.app, "/api/admin/modules", Some(&t.token)).await;
            let module = listed.as_array().unwrap().iter().find(|m| m["id"] == "tv.kroma.stub").unwrap().clone();
            module["configValues"]["url"].clone()
        };

        assert_eq!(put(json!({ "url": "http://portal/?password=x" })).await.0, axum::http::StatusCode::OK);
        assert_eq!(view().await, json!(true));
        assert_eq!(put(json!({ "url": "" })).await.0, axum::http::StatusCode::OK);
        assert_eq!(view().await, json!(true));
        assert_eq!(put(json!({ "url": true })).await.0, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(view().await, json!(true));
        assert_eq!(put(json!({ "url": null })).await.0, axum::http::StatusCode::OK);
        assert_eq!(view().await, json!(null));
    }
}
