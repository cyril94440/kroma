//! The `acquisition` point: the routes this `.kmod` serves behind the core's
//! `/api/requests/:id/search` and `/grab`.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use kroma_module_sdk::host::{port_reply, HostStorage};
use serde::Deserialize;

pub fn acqsearch_routes<S: HostStorage + Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/_port/acquisition/search", post(search_h::<S>))
        .route("/_port/acquisition/search-now", post(search_now_h::<S>))
        .route("/_port/acquisition/grab", post(grab_h::<S>))
        .route("/_port/acquisition/watch-now", post(watch_now_h::<S>))
        .route("/_port/acquisition/watch-now-confirm", post(watch_now_confirm_h::<S>))
        .route("/_port/acquisition/watch-now-reject", post(watch_now_reject_h::<S>))
        .route("/_port/acquisition/watch-now-capable", post(watch_now_capable_h::<S>))
}

async fn watch_now_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(ask): Json<crate::watch_now::Ask>,
) -> Json<Result<crate::watch_now::Reply, String>> {
    port_reply(move || crate::watch_now::watch_now(&host, &ask)).await
}

async fn watch_now_confirm_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(confirmation): Json<crate::watch_now::Confirmation>,
) -> Json<Result<bool, String>> {
    port_reply(move || crate::watch_now::confirm(&host, &confirmation)).await
}

async fn watch_now_reject_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(rejection): Json<crate::watch_now::Rejection>,
) -> Json<Result<bool, String>> {
    port_reply(move || crate::watch_now::reject(&host, &rejection)).await
}

async fn watch_now_capable_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
) -> Json<Result<bool, String>> {
    port_reply(move || Ok(crate::watch_now::capable(&host))).await
}

#[derive(Deserialize)]
struct SearchReq {
    request_id: String,
    #[serde(default)]
    scope: crate::search::SearchScope,
}

async fn search_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<SearchReq>,
) -> Json<Result<serde_json::Value, String>> {
    port_reply(move || {
        Ok(serde_json::to_value(crate::search::interactive_search(
            &host,
            &crate::search::Subject::request(&req.request_id),
            req.scope,
        )?)?)
    })
    .await
}

#[derive(Deserialize)]
struct SearchNowReq {
    kind: String,
    tmdb_id: u64,
    #[serde(default)]
    units: Vec<crate::on_demand::Unit>,
}

async fn search_now_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<SearchNowReq>,
) -> Json<Result<crate::on_demand::Started, String>> {
    port_reply(move || {
        let kind = kroma_module_sdk::domain::RequestKind::parse(&req.kind)
            .ok_or_else(|| anyhow::anyhow!("unknown kind {}", req.kind))?;
        crate::on_demand::start_title_search(&host, kind, req.tmdb_id, &req.units)
    })
    .await
}

#[derive(Deserialize)]
struct GrabReq {
    request_id: String,
    #[serde(default)]
    scope: crate::search::SearchScope,
    guid: String,
    indexer_id: String,
}

async fn grab_h<S: HostStorage + Clone + Send + Sync + 'static>(
    State(host): State<S>,
    Json(req): Json<GrabReq>,
) -> Json<Result<String, String>> {
    port_reply(move || {
        let row = crate::search::grab_cached(
            &host,
            &crate::search::Subject::request(&req.request_id),
            req.scope,
            &req.guid,
            &req.indexer_id,
        )?;
        let id = row.id.clone();
        crate::peers::downloads::activate_later(host, row.id);
        Ok(id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt as _;

    use super::*;

    type DbHost = kroma_module_sdk::host::testing::StubHost;

    fn db_host() -> DbHost {
        let host = DbHost::with_core(kroma_module_sdk::db::testing::temp_pool("acqserve"));
        crate::rules::schema::install(host.store());
        host
    }

    async fn post(path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
        let host = db_host();
        let app = acqsearch_routes::<DbHost>().with_state(host);
        let req = Request::builder()
            .method("POST")
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[tokio::test]
    async fn every_route_is_mounted_where_the_core_calls_them() {
        let (status, _) = post(
            "/_port/acquisition/search",
            serde_json::json!({ "request_id": "r1" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = post(
            "/_port/acquisition/grab",
            serde_json::json!({ "request_id": "r1", "guid": "g", "indexer_id": "i" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = post(
            "/_port/acquisition/search-now",
            serde_json::json!({ "kind": "show", "tmdb_id": 1396 }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn the_watch_now_routes_are_mounted_and_answer_through_the_envelope() {
        let (status, body) = post(
            "/_port/acquisition/watch-now",
            serde_json::json!({ "kind": "movie", "tmdb_id": 1, "user_id": "u" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["Ok"]["status"], "unavailable", "{body}");
        assert_eq!(body["Ok"]["reason"], "noEngine", "{body}");

        let (status, body) = post("/_port/acquisition/watch-now-capable", serde_json::json!({})).await;
        assert_eq!((status, body), (StatusCode::OK, serde_json::json!({ "Ok": false })));

        let (status, body) = post(
            "/_port/acquisition/watch-now-reject",
            serde_json::json!({ "download_id": "d", "kind": "sideways" }),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, serde_json::json!({ "Ok": false })));
    }

    #[tokio::test]
    async fn the_core_can_ask_this_module_to_search_a_title_now() {
        let (status, body) = post(
            "/_port/acquisition/search-now",
            serde_json::json!({ "kind": "show", "tmdb_id": 1396 }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["Ok"]["started"], false, "{body}");
        assert_eq!(body["Ok"]["reason"], "vpnClosed", "{body}");
    }

    #[tokio::test]
    async fn a_search_now_for_a_kind_nobody_has_answers_err_not_a_500() {
        let (status, body) = post(
            "/_port/acquisition/search-now",
            serde_json::json!({ "kind": "sideways", "tmdb_id": 1 }),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["Err"].as_str().is_some(), "{body}");
    }

    #[tokio::test]
    async fn a_scoped_search_is_accepted_and_an_absent_scope_defaults_to_the_whole_request() {
        for scope in [
            serde_json::json!({ "kind": "movie" }),
            serde_json::json!({ "kind": "season", "season": 2 }),
            serde_json::json!({ "kind": "episode", "season": 2, "episode": 5 }),
        ] {
            let (status, body) = post(
                "/_port/acquisition/search",
                serde_json::json!({ "request_id": "r1", "scope": scope }),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{scope}");
            assert!(body["Err"].as_str().is_some(), "{body}");
        }
    }

    #[tokio::test]
    async fn an_unknown_scope_shape_is_rejected_by_the_extractor() {
        let (status, _) = post(
            "/_port/acquisition/search",
            serde_json::json!({ "request_id": "r1", "scope": { "kind": "sideways" } }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn a_search_for_a_request_that_is_not_there_answers_err_not_a_500() {
        let (status, body) = post(
            "/_port/acquisition/search",
            serde_json::json!({ "request_id": "nope" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let err = body["Err"].as_str().expect("an Err envelope");
        assert!(err.contains("request not found"), "{err}");
    }

    #[tokio::test]
    async fn a_grab_for_a_request_that_is_not_there_answers_err_too() {
        let (status, body) = post(
            "/_port/acquisition/grab",
            serde_json::json!({ "request_id": "nope", "guid": "g", "indexer_id": "i" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["Err"].as_str().is_some(), "{body}");
    }

    #[tokio::test]
    async fn a_malformed_body_is_rejected_by_the_extractor() {
        let (status, _) = post("/_port/acquisition/search", serde_json::json!({})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, _) = post(
            "/_port/acquisition/grab",
            serde_json::json!({ "request_id": "r1", "guid": "g" }),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
