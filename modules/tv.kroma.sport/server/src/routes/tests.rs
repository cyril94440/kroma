use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Request, StatusCode};
use axum::Router;
use kroma_module_sdk::domain::{LibraryScope, Permission, User};
use kroma_module_sdk::host::testing::StubHost;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::runtime::tests::support::Rig;

fn user(id: &str, permissions: Vec<Permission>) -> User {
    User {
        id: id.into(),
        email: format!("{id}@example.test"),
        username: id.into(),
        avatar_url: None,
        language: None,
        audio_language: None,
        subtitle_language: None,
        permissions,
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
    let rig = Rig::new("2026-11-03T20:11:00Z");
    let host: StubHost = rig
        .host
        .clone()
        .with_session("ana", user("ana", vec![Permission::Playback]))
        .with_session(
            "cyril",
            user(
                "cyril",
                vec![Permission::Playback, Permission::SettingsManage],
            ),
        )
        .with_session("guest", user("guest", vec![]));
    let router = crate::routes::<StubHost>(rig.sport.clone()).with_state(host);
    App { router, rig }
}

impl App {
    async fn send(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(path)
            .header(AUTHORIZATION, format!("Bearer {token}"));
        let request = match body {
            Some(b) => request
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 22)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

#[tokio::test]
async fn the_state_carries_the_sequence_and_everything_a_client_needs_to_converge() {
    let app = app();

    let (status, body) = app.send("GET", "/state", "ana", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["seq"].is_u64() && body["now"].is_i64());
    assert_eq!(
        (body["me"]["userId"].clone(), body["me"]["admin"].clone()),
        (json!("ana"), json!(false))
    );
    assert!(
        body.get("view").is_some()
            && body.get("match").is_some()
            && body.get("prono").is_some()
            && body["clips"].is_array()
    );
    assert_eq!(body["delay"], json!({"seconds": 60.0, "source": "defaut"}));
}

#[tokio::test]
async fn a_person_without_playback_is_refused_and_admin_routes_need_settings_manage() {
    let app = app();

    let guest = app.send("GET", "/state", "guest", None).await.0;
    let anonymous = app.send("GET", "/state", "nobody", None).await.0;
    let trace = app.send("GET", "/trace", "ana", None).await.0;
    let refresh = app.send("POST", "/refresh", "ana", Some(json!({}))).await.0;
    let allowed = app.send("GET", "/trace", "cyril", None).await.0;

    assert_eq!(
        (guest, anonymous, trace, refresh, allowed),
        (
            StatusCode::FORBIDDEN,
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::FORBIDDEN,
            StatusCode::OK
        )
    );
}

#[tokio::test]
async fn voting_without_a_prono_and_reading_a_clock_without_a_match_are_refused_with_a_reason() {
    let app = app();

    let vote = app
        .send(
            "POST",
            "/prono/vote",
            "ana",
            Some(json!({"id": "espn-1", "score": "1-0"})),
        )
        .await;
    let clock = app
        .send(
            "POST",
            "/calibration",
            "ana",
            Some(json!({"clock": "10:00"})),
        )
        .await;
    let watch = app
        .send(
            "POST",
            "/watch",
            "ana",
            Some(json!({"channelId": "primary:l1p1"})),
        )
        .await;
    let open = app.send("POST", "/open", "ana", Some(json!({}))).await;

    assert_eq!(
        (vote.0, vote.1["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("noOpenProno"))
    );
    assert_eq!(
        (clock.0, clock.1["error"].as_str()),
        (StatusCode::CONFLICT, Some("noMatch"))
    );
    assert_eq!(
        (watch.0, watch.1["outcome"].as_str()),
        (StatusCode::OK, Some("voteStarted"))
    );
    assert_eq!(
        app.rig.room.switches.lock().unwrap().as_slice(),
        [(
            "primary:l1p1".to_string(),
            "ana".to_string(),
            "sport".to_string()
        )]
    );
    assert_eq!(
        (open.0, open.1["accepted"].clone()),
        (StatusCode::OK, json!(true))
    );
}

#[tokio::test]
async fn standings_answer_pending_until_the_background_read_lands_and_the_leaderboard_starts_empty()
{
    let app = app();

    let first = app
        .send("GET", "/standings?competition=Ligue%201", "ana", None)
        .await
        .1;
    let board = app.send("GET", "/leaderboard", "ana", None).await.1;
    let clips = app.send("GET", "/clips", "ana", None).await.1;
    let gone = app.send("DELETE", "/clips/none", "cyril", None).await.0;

    assert_eq!(
        (first["pending"].clone(), first["standings"].clone()),
        (json!(true), Value::Null)
    );
    assert_eq!(
        (board["season"].clone(), clips["clips"].clone()),
        (json!([]), json!([]))
    );
    assert_eq!(gone, StatusCode::NOT_FOUND);
}
