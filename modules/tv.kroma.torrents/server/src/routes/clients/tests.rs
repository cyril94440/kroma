use super::*;

fn answering(kinds: &[&str]) -> Vec<Contribution> {
    kinds
        .iter()
        .map(|kind| Contribution {
            module_id: format!("tv.kroma.engine.{kind}"),
            instance: Some((*kind).to_string()),
            base_url: "http://127.0.0.1:1".into(),
            token: "t".into(),
        })
        .collect()
}

#[test]
fn a_kind_is_valid_when_an_enabled_module_answers_for_it() {
    let answers = answering(&["rqbit", "alldebrid", "transmission"]);

    assert!(is_external_engine(&answers, "alldebrid"));
    assert!(is_external_engine(&answers, "transmission"));
}

#[test]
fn a_kind_nobody_answers_for_is_refused() {
    let answers = answering(&["rqbit"]);

    assert!(!is_external_engine(&answers, "qbittorrent"));
    assert!(!is_external_engine(&answers, ""));
    assert!(!is_external_engine(&[], "alldebrid"));
}

#[test]
fn the_embedded_kind_is_never_created_through_this_route() {
    assert!(!is_external_engine(&answering(&["rqbit"]), "rqbit"));
}

#[test]
fn a_contribution_with_no_instance_answers_for_no_kind() {
    let mut answers = answering(&["alldebrid"]);
    answers[0].instance = None;

    assert!(!is_external_engine(&answers, "alldebrid"));
}

mod vetting {
    use std::sync::Arc;

    use axum::routing::post;
    use axum::{Json, Router};
    use kroma_module_sdk::host::testing::StubHost;
    use kroma_module_sdk::host::HostCtx as _;
    use kroma_module_sdk::testing::serve;

    use super::super::*;
    use crate::downloads::test_support::host_over;

    async fn manager(test_answer: Result<String, String>) -> (Arc<DownloadManager>, Arc<std::sync::atomic::AtomicU32>) {
        let asked = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counted = asked.clone();
        let router = Router::new().route(
            "/_port/tv.kroma.torrents/client/test",
            post(move |Json(_): Json<serde_json::Value>| {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let answer = test_answer.clone();
                async move { Json(answer) }
            }),
        );
        let resolve = serve(router, ()).await;
        let (base, token) = resolve().expect("the fake engine is up");
        let host: StubHost = host_over("torrents-vet", &[]).with_point(DOWNLOAD_CLIENT, Some("fake"), &base, &token);
        let manager = DownloadManager::new(
            Arc::new(host.clone()),
            host.data_dir(),
            host.db().clone(),
            host.store().clone(),
        );
        (manager, asked)
    }

    fn row(url: &str, password: &str) -> DownloadClientRow {
        DownloadClientRow {
            id: "c".into(),
            kind: "fake".into(),
            name: "fake".into(),
            url: url.into(),
            username: String::new(),
            password: password.into(),
            enabled: true,
            priority: 0,
            created_at: 1,
        }
    }

    #[tokio::test]
    async fn a_definition_missing_its_secret_is_refused_with_the_engines_own_reason() {
        let (manager, asked) = manager(Err("an API key is required".into())).await;

        let refused = vet_new_client(manager, row("", "")).await.unwrap_err();

        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        let body = axum::body::to_bytes(refused.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("an API key is required"));
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_complete_definition_is_saved_without_a_probe() {
        let (manager, asked) = manager(Err("never asked".into())).await;

        let kept = vet_new_client(manager, row("http://x", "pw")).await.unwrap();

        assert_eq!(kept.url, "http://x");
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_incomplete_definition_the_engine_accepts_is_saved() {
        let (manager, _) = manager(Ok("v1".into())).await;

        assert!(vet_new_client(manager, row("http://x", "")).await.is_ok());
    }
}
