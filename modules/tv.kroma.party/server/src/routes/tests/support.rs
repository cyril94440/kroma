use std::sync::Arc;

use axum::body::Body;
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE};
use axum::http::{Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tower::ServiceExt;

use kroma_module_sdk::domain::{LibraryScope, Permission, User};
use kroma_module_sdk::host::testing::StubHost;
use kroma_module_sdk::host::{HostCtx, HostStorage};

use crate::live::{Live, Status, Switched, Viewer};
use crate::party::Party;

#[derive(Default)]
pub struct Live1 {
    pub viewers: std::sync::Mutex<Vec<Viewer>>,
    pub switched: std::sync::Mutex<Vec<String>>,
}

impl Live for Live1 {
    fn status(&self) -> anyhow::Result<Status> {
        Ok(Status::default())
    }

    fn presence(&self) -> anyhow::Result<Vec<Viewer>> {
        Ok(self.viewers.lock().unwrap().clone())
    }

    fn switch(&self, id: &str, _by: &str, _why: &str) -> anyhow::Result<Switched> {
        self.switched.lock().unwrap().push(id.into());
        Ok(Switched { switched: true, ..Switched::default() })
    }

    fn proposal(&self, _id: &str, _accept: bool) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn resolve(&self, _q: &str, _l: u32) -> anyhow::Result<Vec<crate::live::Candidate>> {
        Ok(Vec::new())
    }
}

pub struct App {
    pub router: Router,
    pub user_routes: Router,
    pub point_routes: Router,
    pub live: Arc<Live1>,
    pub party: Arc<Party>,
}

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

pub fn app() -> App {
    let host = StubHost::with_db("party-routes")
        .with_session("mamie", user("mamie", false))
        .with_session("papi", user("papi", false))
        .with_session("cyril", user("cyril", true));
    host.store().get().unwrap().execute_batch(crate::store::MIGRATIONS).unwrap();
    let live = Arc::new(Live1::default());
    let ctx: Arc<dyn HostCtx> = Arc::new(host.clone());
    let party = Party::new(ctx, host.store().clone(), live.clone());
    let user_routes = crate::routes::<StubHost>(party.clone()).with_state(host.clone());
    let point_routes = crate::point_routes::<StubHost>(party.clone()).with_state(host.clone());
    let router = crate::routes::<StubHost>(party.clone())
        .merge(crate::point_routes::<StubHost>(party.clone()))
        .with_state(host);
    App { router, user_routes, point_routes, live, party }
}

impl App {
    pub fn watching(&self, names: &[&str]) {
        *self.live.viewers.lock().unwrap() =
            names.iter().map(|n| Viewer { user_id: (*n).into(), name: (*n).into(), since: 0 }).collect();
    }

    pub async fn send(&self, method: &str, path: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            request = request.header(AUTHORIZATION, format!("Bearer {t}"));
        }
        let request = match body {
            Some(b) => request.header(CONTENT_TYPE, "application/json").body(Body::from(b.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str, token: &str) -> (StatusCode, Value) {
        self.send("GET", path, Some(token), None).await
    }

    pub async fn post(&self, path: &str, token: &str, body: Value) -> (StatusCode, Value) {
        self.send("POST", path, Some(token), Some(body)).await
    }

    pub async fn port(&self, method: &str, body: Value) -> Value {
        let (status, json) = self.send("POST", &format!("/_port/tv.kroma.party/room/{method}"), None, Some(body)).await;
        assert_eq!(status, StatusCode::OK);
        json
    }
}
