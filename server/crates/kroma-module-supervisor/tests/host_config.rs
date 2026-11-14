use axum::http::StatusCode;
use kroma_module_host::testing::StubHost;
use serde_json::{Value, json};

mod test_support;

use test_support::settings::refusal;
use test_support::{FABRIC, Modules};

const DOWNLOADS: &str = "tv.kroma.downloads";
const NOTES: &str = "tv.kroma.notes";

fn installed() -> Modules {
    Modules::new("host-config")
        .configured(
            DOWNLOADS,
            r#"[
                { "key": "cacheMaxGb", "label": "config.cacheMaxGb", "type": "number", "default": "50" },
                { "key": "apiKey", "label": "config.apiKey", "type": "string", "secret": true },
                { "key": "region", "label": "config.region", "type": "string" }
            ]"#,
        )
        .configured(
            NOTES,
            r#"[{ "key": "theme", "label": "config.theme", "type": "string", "default": "dark" }]"#,
        )
}

async fn read(modules: &Modules, host: StubHost, token: &str, key: &str) -> (StatusCode, String) {
    modules
        .get(host, &format!("/_host/config?key={key}"), Some(token))
        .await
}

fn value(body: &str) -> Value {
    serde_json::from_str::<Value>(body).unwrap()["value"].clone()
}

#[tokio::test]
async fn a_value_the_operator_saved_comes_back_to_its_own_module() {
    let modules = installed();
    let host = StubHost::new().with_module_config(DOWNLOADS, "cacheMaxGb", json!(5));

    let (status, body) = read(&modules, host, &modules.token(DOWNLOADS), "cacheMaxGb").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value(&body), json!(5));
}

#[tokio::test]
async fn a_field_nobody_saved_answers_the_manifest_default() {
    let modules = installed();

    let (status, body) = read(
        &modules,
        StubHost::new(),
        &modules.token(DOWNLOADS),
        "cacheMaxGb",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value(&body), json!("50"));
}

#[tokio::test]
async fn a_declared_field_with_neither_value_nor_default_is_null() {
    let modules = installed();

    let (status, body) = read(&modules, StubHost::new(), &modules.token(DOWNLOADS), "region").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value(&body), Value::Null);
}

#[tokio::test]
async fn a_secret_field_reaches_the_module_that_owns_it() {
    let modules = installed();
    let host = StubHost::new().with_module_config(DOWNLOADS, "apiKey", json!("k-123"));

    let (status, body) = read(&modules, host, &modules.token(DOWNLOADS), "apiKey").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(value(&body), json!("k-123"));
}

#[tokio::test]
async fn a_key_the_manifest_does_not_declare_is_refused_and_the_stored_value_stays_back() {
    let modules = installed();
    let host = StubHost::new().with_module_config(DOWNLOADS, "undeclared", json!("hidden-value"));

    let (status, body) = read(&modules, host, &modules.token(DOWNLOADS), "undeclared").await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        refusal(&body),
        r#"config: "undeclared" is not declared; add it to config in module.json"#
    );
    assert!(!body.contains("hidden-value"));
}

#[tokio::test]
async fn another_modules_config_is_unreachable_even_by_its_key() {
    let modules = installed();
    let host = StubHost::new()
        .with_module_config(DOWNLOADS, "cacheMaxGb", json!(5))
        .with_module_config(NOTES, "theme", json!("light"));

    let (status, body) = read(&modules, host.clone(), &modules.token(NOTES), "cacheMaxGb").await;
    let (own_status, own_body) = read(&modules, host, &modules.token(NOTES), "theme").await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.contains('5'));
    assert_eq!(own_status, StatusCode::OK);
    assert_eq!(value(&own_body), json!("light"));
}

#[tokio::test]
async fn a_caller_holding_only_the_fabric_token_has_no_config() {
    let modules = installed();
    let host = StubHost::new().with_module_config(DOWNLOADS, "cacheMaxGb", json!(5));

    let (status, body) = read(&modules, host, FABRIC, "cacheMaxGb").await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!body.contains('5'));
}

#[tokio::test]
async fn a_caller_with_no_token_is_unauthorised() {
    let modules = installed();

    let (status, _) = modules
        .get(StubHost::new(), "/_host/config?key=cacheMaxGb", None)
        .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_refusal_is_logged_without_the_stored_value() {
    let modules = installed();
    let host = StubHost::new().with_module_config(DOWNLOADS, "apiKey", json!("k-123"));

    read(&modules, host, &modules.token(DOWNLOADS), "undeclared").await;

    let said = modules.said.lock().unwrap().join("\n");
    assert!(said.contains("undeclared"));
    assert!(!said.contains("k-123"));
}
