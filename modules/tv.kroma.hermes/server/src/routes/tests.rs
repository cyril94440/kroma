use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Request, StatusCode};
use axum::Router;
use kroma_module_sdk::domain::{LibraryScope, Permission, User};
use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::model::ModelError;
use crate::testing::{Rig, Scripted, Step};

fn user(id: &str, admin: bool) -> User {
    User {
        id: id.into(),
        email: format!("{id}@example.test"),
        username: id.into(),
        avatar_url: None,
        language: None,
        audio_language: None,
        subtitle_language: None,
        permissions: if admin { vec![Permission::Playback, Permission::SettingsManage] } else { vec![Permission::Playback] },
        libraries: LibraryScope::All,
        created_at: "2024-01-01T00:00:00Z".into(),
        has_pin: false,
    }
}

struct App {
    router: Router,
    rig: Rig,
}

fn app() -> App {
    let rig = Rig::new();
    let host: StubHost = rig.host.clone().with_session("cyril", user("cyril", true)).with_session("mamie", user("mamie", false));
    let router = crate::routes::<StubHost>(rig.agent.clone()).with_state(host);
    App { router, rig }
}

impl App {
    async fn send(&self, method: &str, path: &str, token: &str, body: Option<Value>) -> (StatusCode, Value) {
        let request = Request::builder().method(method).uri(path).header(AUTHORIZATION, format!("Bearer {token}"));
        let request = match body {
            Some(b) => request.header(CONTENT_TYPE, "application/json").body(Body::from(b.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 22).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn every_route_is_for_administrators_only() {
    let app = app();
    for (method, path) in [("GET", "/status"), ("GET", "/audit"), ("GET", "/rules"), ("GET", "/lessons"), ("GET", "/suggestions"), ("GET", "/notes"), ("POST", "/model/test"), ("GET", "/model/list")] {
        let (status, _) = app.send(method, path, "mamie", Some(json!({}))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
    }
}

#[tokio::test]
async fn the_status_page_reports_the_agent_and_the_pause_switch_works() {
    let app = app();
    let (status, body) = app.send("GET", "/status", "cyril", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((body["enabled"].as_bool(), body["paused"].as_bool(), body["configured"].as_bool()), (Some(true), Some(false), Some(false)));
    app.send("POST", "/pause", "cyril", Some(json!({ "paused": true }))).await;
    assert_eq!(app.send("GET", "/status", "cyril", None).await.1["paused"], true);
}

#[tokio::test]
async fn the_audit_log_is_readable_by_genre_and_page() {
    let app = app();
    app.rig.deps.audit("chat", "gate", json!({ "a": 1 }));
    app.rig.deps.audit("llm", "call", json!({ "b": 2 }));
    let (_, all) = app.send("GET", "/audit", "cyril", None).await;
    assert_eq!(all["rows"].as_array().unwrap().len(), 2);
    assert_eq!(all["genres"], json!(["chat", "llm"]));
    let (_, chat) = app.send("GET", "/audit?genre=chat&limit=5", "cyril", None).await;
    assert_eq!(chat["rows"][0]["event"], "gate");
    let before = all["rows"][0]["id"].as_i64().unwrap();
    let (_, older) = app.send("GET", &format!("/audit?before={before}"), "cyril", None).await;
    assert_eq!(older["rows"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn house_rules_are_edited_reset_and_bounded() {
    let app = app();
    let (_, shipped) = app.send("GET", "/rules", "cyril", None).await;
    assert_eq!(shipped["customised"], false);
    assert!(shipped["text"].as_str().unwrap().contains("Chaînes FRANÇAISES en priorité"));
    assert_eq!(app.send("PUT", "/rules", "cyril", Some(json!({ "text": "- Nouvelle règle" }))).await.0, StatusCode::OK);
    let (_, edited) = app.send("GET", "/rules", "cyril", None).await;
    assert_eq!((edited["text"].as_str(), edited["customised"].as_bool()), (Some("- Nouvelle règle"), Some(true)));
    assert_eq!(app.send("PUT", "/rules", "cyril", Some(json!({ "text": "x".repeat(20_001) }))).await.0, StatusCode::BAD_REQUEST);
    app.send("POST", "/rules/reset", "cyril", Some(json!({}))).await;
    assert_eq!(app.send("GET", "/rules", "cyril", None).await.1["customised"], false);
}

#[tokio::test]
async fn lessons_suggestions_and_notes_can_be_read_and_removed() {
    let app = app();
    let store = &app.rig.deps.store;
    store.add_lesson("[01/01/2026] Une leçon", 1, 30).unwrap();
    store.add_suggestion("bug", "Un titre", "Un détail", 1, 120).unwrap();
    store.add_note("u-mamie", "Mamie", "Aime le rugby", 1, 12).unwrap();
    let (_, lessons) = app.send("GET", "/lessons", "cyril", None).await;
    let id = lessons["lessons"][0]["id"].as_i64().unwrap();
    assert_eq!(app.send("DELETE", &format!("/lessons/{id}"), "cyril", None).await.1["ok"], true);
    let (_, suggestions) = app.send("GET", "/suggestions", "cyril", None).await;
    assert_eq!(suggestions["suggestions"][0]["title"], "Un titre");
    let sid = suggestions["suggestions"][0]["id"].as_i64().unwrap();
    assert_eq!(app.send("DELETE", &format!("/suggestions/{sid}"), "cyril", None).await.1["ok"], true);
    let (_, notes) = app.send("GET", "/notes", "cyril", None).await;
    assert_eq!(notes["notes"][0], json!({ "userId": "u-mamie", "name": "Mamie", "note": "Aime le rugby" }));
    assert_eq!(app.send("DELETE", "/notes/u-mamie", "cyril", None).await.1["removed"], 1);
    assert!(store.lessons().unwrap().is_empty() && store.suggestions().unwrap().is_empty() && store.all_notes().unwrap().is_empty());
}

#[tokio::test]
async fn testing_the_model_reports_success_failure_and_not_configured() {
    let app = app();
    let (status, none) = app.send("POST", "/model/test", "cyril", Some(json!({}))).await;
    assert_eq!((status, none["ok"].as_bool()), (StatusCode::OK, Some(false)));
    assert!(none["error"].as_str().unwrap().contains("not configured"));
    app.rig.use_model(std::sync::Arc::new(Scripted::new(vec![])));
    let (_, ok) = app.send("POST", "/model/test", "cyril", Some(json!({}))).await;
    assert_eq!(ok["ok"], true);
    assert_eq!(ok["answer"], "ok");
    let (_, list) = app.send("GET", "/model/list", "cyril", None).await;
    assert_eq!(list["models"], json!(["scripted"]));
    let _ = Step::Fail(ModelError::NotConfigured);
}
