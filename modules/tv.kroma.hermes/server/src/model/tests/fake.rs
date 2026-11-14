use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;

#[derive(Clone, Debug, Default)]
pub struct Req {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

#[derive(Clone)]
pub enum Reply {
    Json(u16, Value),
    Raw(u16, String),
    Hang(Duration),
}

pub struct FakeModel {
    pub base: String,
    calls: Arc<Mutex<Vec<Req>>>,
}

impl FakeModel {
    pub fn start(handler: impl Fn(&Req, usize) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let handler = Arc::new(handler);
        let log = calls.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let (handler, log) = (handler.clone(), log.clone());
                std::thread::spawn(move || serve(stream, &*handler, &log));
            }
        });
        Self { base, calls }
    }

    pub fn scripted(replies: Vec<Reply>) -> Self {
        Self::start(move |_, n| replies[n.min(replies.len() - 1)].clone())
    }

    pub fn requests(&self) -> Vec<Req> {
        self.calls.lock().unwrap().clone()
    }

    pub fn count(&self) -> usize {
        self.requests().len()
    }
}

fn serve(stream: std::net::TcpStream, handler: &dyn Fn(&Req, usize) -> Reply, log: &Mutex<Vec<Req>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let Some(req) = read(&mut reader) else { return };
    let index = {
        let mut calls = log.lock().unwrap();
        calls.push(req.clone());
        calls.len() - 1
    };
    let (status, body) = match handler(&req, index) {
        Reply::Json(status, value) => (status, value.to_string()),
        Reply::Raw(status, text) => (status, text),
        Reply::Hang(wait) => {
            std::thread::sleep(wait);
            return;
        }
    };
    let mut out = stream;
    let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n", body.len());
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(body.as_bytes());
    let _ = out.flush();
}

fn read(reader: &mut BufReader<std::net::TcpStream>) -> Option<Req> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut req = Req { method, path, ..Req::default() };
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).ok()? == 0 || header == "\r\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            if name == "content-length" {
                length = value.trim().parse().unwrap_or(0);
            }
            req.headers.insert(name, value.trim().to_string());
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    req.body = serde_json::from_slice(&body).unwrap_or(Value::Null);
    Some(req)
}
