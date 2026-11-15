use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::Value;

type Routes = Arc<Mutex<BTreeMap<String, (u16, String)>>>;

pub struct FakeEspn {
    pub base: String,
    routes: Routes,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeEspn {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let routes: Routes = Arc::default();
        let calls: Arc<Mutex<Vec<String>>> = Arc::default();
        let (served, log) = (routes.clone(), calls.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let (served, log) = (served.clone(), log.clone());
                std::thread::spawn(move || answer(stream, &served, &log));
            }
        });
        Self {
            base,
            routes,
            calls,
        }
    }

    pub fn serve(&self, path: &str, body: &Value) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_string(), (200, body.to_string()));
    }

    pub fn fail(&self, path: &str, status: u16) {
        self.routes
            .lock()
            .unwrap()
            .insert(path.to_string(), (status, "{}".into()));
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

fn answer(stream: std::net::TcpStream, routes: &Routes, calls: &Arc<Mutex<Vec<String>>>) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut request = String::new();
    if reader.read_line(&mut request).is_err() {
        return;
    }
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) <= 2 {
            break;
        }
    }
    let target = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    calls.lock().unwrap().push(target.clone());
    let bare = target.split('?').next().unwrap_or("").to_string();
    let found = {
        let table = routes.lock().unwrap();
        table.get(&target).or_else(|| table.get(&bare)).cloned()
    };
    let (status, body) = found.unwrap_or((404, "{}".into()));
    let mut out = stream;
    let _ = write!(out, "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}
