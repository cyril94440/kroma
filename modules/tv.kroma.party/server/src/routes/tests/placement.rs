use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use super::support::*;

async fn status(router: &axum::Router, method: &str, path: &str) -> StatusCode {
    let request = Request::builder().method(method).uri(path).body(Body::empty()).unwrap();
    router.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn point_routes_hold_only_the_port_surface_the_host_token_guards() {
    let app = app();
    assert_eq!(status(&app.point_routes, "POST", "/_port/tv.kroma.party/room/say").await, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(status(&app.point_routes, "GET", "/state").await, StatusCode::NOT_FOUND);
    assert_eq!(status(&app.point_routes, "POST", "/chat").await, StatusCode::NOT_FOUND);
    assert_eq!(status(&app.point_routes, "GET", "/sync/info").await, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn user_routes_hold_no_port_route_so_a_session_bearer_is_all_they_need() {
    let app = app();
    assert_eq!(status(&app.user_routes, "POST", "/_port/tv.kroma.party/room/say").await, StatusCode::NOT_FOUND);
    assert_eq!(status(&app.user_routes, "GET", "/state").await, StatusCode::UNAUTHORIZED);
    let (code, body) = app.get("/state", "mamie").await;
    assert_eq!(code, StatusCode::OK);
    assert!(body["me"]["userId"].is_string());
}

#[test]
fn the_module_hands_its_user_routes_to_the_host_through_admin_routes() {
    use kroma_module_sdk::host::testing::StubHost;
    use kroma_module_sdk::host::ServerModule;
    let app = app();
    let bare = StubHost::with_db("placement-bare");
    assert!(crate::PartyModule.admin_routes(&bare).is_none());
    let wired = StubHost::with_db("placement-wired").with_service(app.party.clone());
    assert!(crate::PartyModule.admin_routes(&wired).is_some());
}
