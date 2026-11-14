//! The only way an ffmpeg or ffprobe process reads a title that is not a file
//! here: a plain-HTTP listener on `127.0.0.1`, apart from the public one and
//! never part of its router, that serves a stub through [`Sources::serve`], so a
//! read has the same policy, vetting and journal as direct play and ffmpeg is
//! never handed an address a module supplied.
//!
//! A request names a per-process secret and a grant. A grant stands for one
//! stub and lives as long as the [`Grant`] handle the input carries, with a
//! hard expiry besides, so a command line that leaks into a log or a process
//! list opens nothing once its session is over and never opens another file.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Path, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use kroma_media::Unmetered;

use super::Sources;

const GRANT_LIFETIME: Duration = Duration::from_secs(12 * 60 * 60);

pub(super) struct Table(Mutex<HashMap<String, (String, Instant)>>);

impl Table {
    pub(super) fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, (String, Instant)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn stub_of(&self, grant: &str) -> Option<String> {
        let mut held = self.lock();
        match held.get(grant) {
            Some((stub, expires)) if Instant::now() < *expires => Some(stub.clone()),
            Some(_) => {
                held.remove(grant);
                None
            }
            None => None,
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// Permission to read one stub through the relay, revoked when dropped.
pub struct Grant {
    id: String,
    table: Arc<Table>,
}

impl Grant {
    pub(super) fn id(&self) -> String {
        self.id.clone()
    }
}

impl Drop for Grant {
    fn drop(&mut self) {
        self.table.lock().remove(&self.id);
    }
}

struct Bound {
    port: u16,
    _stop: tokio::sync::oneshot::Sender<()>,
}

pub(super) struct Loopback {
    secret: String,
    table: Arc<Table>,
    lifetime: Duration,
    bound: Mutex<Option<Bound>>,
}

impl Loopback {
    pub(super) fn new() -> Self {
        Self::lasting(GRANT_LIFETIME)
    }

    pub(super) fn lasting(lifetime: Duration) -> Self {
        Self {
            secret: kroma_primitives::random_token(),
            table: Arc::new(Table::new()),
            lifetime,
            bound: Mutex::new(None),
        }
    }

    pub(super) fn secret(&self) -> &str {
        &self.secret
    }

    #[cfg(test)]
    pub(super) fn table(&self) -> &Arc<Table> {
        &self.table
    }

    /// A grant for the stub at `abs`, and the port the relay listens on, which is
    /// bound the first time one is asked for.
    pub(super) fn grant(&self, sources: &Weak<Sources>, abs: &str) -> io::Result<(u16, Grant)> {
        let port = self.port(sources)?;
        let id = kroma_primitives::random_token();
        self.table.lock().insert(
            id.clone(),
            (abs.to_string(), Instant::now() + self.lifetime),
        );
        Ok((
            port,
            Grant {
                id,
                table: self.table.clone(),
            },
        ))
    }

    fn port(&self, sources: &Weak<Sources>) -> io::Result<u16> {
        let mut bound = self.bound.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(open) = bound.as_ref() {
            return Ok(open.port);
        }
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let shared = Arc::new(Shared {
            secret: self.secret.clone(),
            table: self.table.clone(),
            sources: sources.clone(),
        });
        std::thread::Builder::new()
            .name("source-relay".into())
            .spawn(move || run(listener, shared, stopped))?;
        *bound = Some(Bound { port, _stop: stop });
        Ok(port)
    }
}

struct Shared {
    secret: String,
    table: Arc<Table>,
    sources: Weak<Sources>,
}

fn run(listener: TcpListener, shared: Arc<Shared>, stopped: tokio::sync::oneshot::Receiver<()>) {
    let Ok(runtime) = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    else {
        return;
    };
    runtime.block_on(async move {
        let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
            return;
        };
        let app = Router::new()
            .route("/{secret}/{grant}/{name}", get(serve).head(serve))
            .with_state(shared)
            .into_make_service_with_connect_info::<SocketAddr>();
        tokio::select! {
            _ = axum::serve(listener, app) => {}
            _ = stopped => {}
        }
    });
}

/// The stub a request may read, or `None`: from anywhere but loopback, with a
/// secret that is not this process's, or with a grant that is unknown or over.
pub(super) fn authorise(
    table: &Table,
    expected: &str,
    peer: IpAddr,
    secret: &str,
    grant: &str,
) -> Option<String> {
    if !peer.is_loopback() || !same(secret, expected) {
        return None;
    }
    table.stub_of(grant)
}

fn same(given: &str, expected: &str) -> bool {
    let (given, expected) = (given.as_bytes(), expected.as_bytes());
    let mut difference = u8::from(given.len() != expected.len());
    for (a, b) in given.iter().zip(expected) {
        difference |= a ^ b;
    }
    difference == 0
}

async fn serve(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((secret, grant, _name)): Path<(String, String, String)>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Some(stub) = authorise(&shared.table, &shared.secret, peer.ip(), &secret, &grant) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(sources) = shared.sources.upgrade() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    match sources
        .serve(&stub, false, method == Method::HEAD, &headers, Unmetered)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::debug!(%error, "the relay could not serve a title to a local reader");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
