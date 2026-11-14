//! A title held by a module, for the byte routes: the module is a socket on this
//! machine answering `source/locate` with whatever a test last told it, and the
//! host the bytes come from is a [`RangedServer`] serving a real file.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use kroma_module_host::Contribution;
use kroma_primitives::remote_file::{self, RemoteFile};
use kroma_testing::RangedServer;
use serde_json::{json, Value};

use super::{build_app_wired, TestApp};

pub const MODULE_ID: &str = "tv.example.debrid";
pub const SOURCE_ID: &str = "debrid";

/// The module standing behind a `source` contribution.
pub struct FakeSource {
    port: u16,
    answer: Arc<Mutex<Value>>,
    asked: Arc<Mutex<Vec<Value>>>,
    stopped: Arc<AtomicBool>,
}

impl FakeSource {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        listener.set_nonblocking(true).expect("poll the listener");
        let fake = Self {
            port: listener.local_addr().expect("a bound address").port(),
            answer: Arc::new(Mutex::new(json!({ "Err": "not told what to answer" }))),
            asked: Arc::default(),
            stopped: Arc::default(),
        };
        let (answer, asked, stopped) = (fake.answer.clone(), fake.asked.clone(), fake.stopped.clone());
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let (answer, asked) = (answer.clone(), asked.clone());
                std::thread::spawn(move || reply(stream, &answer, &asked));
            }
        });
        fake
    }

    pub fn answers_url(&self, url: &str, size: u64) {
        *self.answer.lock().unwrap() = json!({ "Ok": { "path": null, "url": url, "size": size } });
    }

    pub fn answers_path(&self, path: &Path, size: u64) {
        *self.answer.lock().unwrap() = json!({ "Ok": { "path": path, "url": null, "size": size } });
    }

    pub fn answers_error(&self, message: &str) {
        *self.answer.lock().unwrap() = json!({ "Err": message });
    }

    pub fn asked(&self) -> Vec<Value> {
        self.asked.lock().unwrap().clone()
    }
}

impl Drop for FakeSource {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
    }
}

fn reply(stream: TcpStream, answer: &Mutex<Value>, asked: &Mutex<Vec<Value>>) {
    let Ok(read_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(read_half);
    let mut out = stream;
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap_or(0) == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; length];
    let _ = reader.read_exact(&mut body);
    if let Ok(parsed) = serde_json::from_slice(&body) {
        asked.lock().unwrap().push(parsed);
    }
    let payload = answer.lock().unwrap().to_string();
    let _ = write!(
        out,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

/// The app with a `source` contribution backed by a [`FakeSource`], and a
/// source that is willing to read from a loopback socket.
pub fn test_app_with_source() -> (TestApp, FakeSource) {
    let fake = FakeSource::start();
    let base = format!("http://127.0.0.1:{}", fake.port);
    let contributions: kroma_engine::state::Contributions = Arc::new(move |point| {
        if point != "source" {
            return Vec::new();
        }
        vec![Contribution {
            module_id: MODULE_ID.into(),
            instance: Some(SOURCE_ID.into()),
            base_url: base.clone(),
            token: "port-token".into(),
        }]
    });
    let app = build_app_wired(None, &[], |_| (), contributions);
    app.state.sources.allow_loopback();
    (app, fake)
}

/// A scanned stub in a library under the data dir, standing for `real`, which a
/// [`RangedServer`] serves.
pub struct Stubbed {
    pub item: String,
    pub file: String,
    pub abs: String,
    pub cdn: RangedServer,
}

pub async fn stubbed_movie(t: &TestApp, real: &Path, name: &str) -> Stubbed {
    let library = t.state.config.data_dir.join("library");
    let stub = library.join("Remote Movie (2020)").join("Remote Movie (2020).kremote");
    let size = std::fs::metadata(real).expect("the real file").len();
    remote_file::write(
        &stub,
        &RemoteFile {
            source: SOURCE_ID.into(),
            reference: "7:Remote.Movie.2020.mp4".into(),
            name: name.into(),
            size,
        },
    )
    .expect("write the stub");
    let state = t.state.clone();
    let root = library.to_string_lossy().into_owned();
    let scanned = tokio::task::spawn_blocking(move || {
        let mut defs = crate::services::settings::library_defs(&state.settings, &state.config);
        defs.push(crate::services::settings::LibraryDef {
            id: "remote-lib".into(),
            name: "Remote".into(),
            kind: "movies".into(),
            folders: vec![root],
            auto_scan: false,
        });
        crate::services::settings::set_library_defs(&state.settings, &state.db, &defs);
        crate::services::scan::rescan_sync(&state)
    })
    .await
    .expect("scan task")
    .expect("scan the library");
    let crate::services::scan::Scanned::Applied(data) = scanned else {
        panic!("the library produced no items");
    };
    let item = data
        .items
        .iter()
        .find(|item| item.title == "Remote Movie")
        .expect("the stub was scanned");
    let file = item.files[0].clone();
    Stubbed {
        item: item.id.clone(),
        file: file.id,
        abs: file.abs_path.expect("a scanned path"),
        cdn: RangedServer::serve(real),
    }
}

/// Probe the stub's file the way the background pass does, against what the
/// fake source answers now.
pub async fn probe(t: &TestApp, stubbed: &Stubbed) -> anyhow::Result<()> {
    let state = t.state.clone();
    let (file, abs, item) = (stubbed.file.clone(), stubbed.abs.clone(), stubbed.item.clone());
    tokio::task::spawn_blocking(move || {
        kroma_engine::infra::probe::probe_one(&state.db, true, &state.sources, &state.events, &file, &abs, &item)
    })
    .await
    .expect("probe task")
}
