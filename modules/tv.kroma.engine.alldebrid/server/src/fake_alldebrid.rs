use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

#[derive(Clone, Debug, Default)]
pub(crate) struct Req {
    #[allow(dead_code)]
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl Req {
    pub(crate) fn form(&self) -> HashMap<String, String> {
        parse_pairs(&String::from_utf8_lossy(&self.body))
    }

    pub(crate) fn form_all(&self, name: &str) -> Vec<String> {
        String::from_utf8_lossy(&self.body)
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .filter(|(key, _)| decode(key) == name)
            .map(|(_, value)| decode(value))
            .collect()
    }

    pub(crate) fn range(&self) -> Option<(u64, u64)> {
        let spec = self.headers.get("range")?.strip_prefix("bytes=")?;
        let (from, to) = spec.split_once('-')?;
        Some((from.parse().ok()?, to.parse().ok()?))
    }
}

pub(crate) struct Resp {
    status: u16,
    body: Vec<u8>,
    extra: Vec<(String, String)>,
}

impl Resp {
    pub(crate) fn data(data: Value) -> Self {
        Self::raw(
            200,
            json!({ "status": "success", "data": data })
                .to_string()
                .into_bytes(),
        )
    }

    pub(crate) fn failure(code: &str, message: &str) -> Self {
        let body = json!({ "status": "error", "error": { "code": code, "message": message } });
        Self::raw(200, body.to_string().into_bytes())
    }

    pub(crate) fn raw(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            body,
            extra: Vec::new(),
        }
    }

    pub(crate) fn header(mut self, name: &str, value: String) -> Self {
        self.extra.push((name.to_string(), value));
        self
    }

    pub(crate) fn bytes(req: &Req, whole: &[u8], honour_range: bool) -> Self {
        match req.range().filter(|_| honour_range) {
            Some((from, to)) => {
                let end = (to as usize + 1).min(whole.len());
                let start = (from as usize).min(end);
                Self::raw(206, whole[start..end].to_vec()).header(
                    "Content-Range",
                    format!("bytes {start}-{}/{}", end.saturating_sub(1), whole.len()),
                )
            }
            None => Self::raw(200, whole.to_vec()),
        }
    }
}

pub(crate) struct Fake {
    pub base: String,
    calls: Arc<Mutex<Vec<Req>>>,
}

impl Fake {
    pub(crate) fn start(handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> Self {
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

    pub(crate) fn calls(&self) -> Vec<Req> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn calls_to(&self, path: &str) -> Vec<Req> {
        self.calls()
            .into_iter()
            .filter(|call| call.path == path)
            .collect()
    }
}

fn serve(stream: std::net::TcpStream, handler: &dyn Fn(&Req) -> Resp, log: &Mutex<Vec<Req>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let Some(req) = read(&mut reader) else { return };
    log.lock().unwrap().push(req.clone());
    let resp = handler(&req);
    let mut out = stream;
    let head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
        resp.status,
        resp.body.len()
    );
    let extra: String = resp
        .extra
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect();
    let head = head.replacen("Connection: close\r\n", &format!("{extra}Connection: close\r\n"), 1);
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(&resp.body);
    let _ = out.flush();
}

fn read(reader: &mut BufReader<std::net::TcpStream>) -> Option<Req> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let mut req = Req {
        method,
        path: path.to_string(),
        query: parse_pairs(query),
        ..Req::default()
    };
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
    req.body = vec![0; length];
    reader.read_exact(&mut req.body).ok()?;
    Some(req)
}

fn parse_pairs(text: &str) -> HashMap<String, String> {
    text.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'+' => out.push(b' '),
            b'%' if at + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        at += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        at += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn host_of(req: &Req) -> String {
    req.headers.get("host").cloned().unwrap_or_default()
}

pub(crate) fn scripted(
    tree: Value,
    files: Vec<(&'static str, Vec<u8>)>,
    honour_range: bool,
) -> Fake {
    Script::new(tree, files, honour_range).start()
}

pub(crate) struct Script {
    tree: Value,
    files: HashMap<&'static str, Vec<u8>>,
    honour_range: bool,
    pub status_code: Arc<std::sync::atomic::AtomicU8>,
    pub valid_key: Arc<Mutex<Option<String>>>,
}

impl Script {
    pub(crate) fn new(
        tree: Value,
        files: Vec<(&'static str, Vec<u8>)>,
        honour_range: bool,
    ) -> Self {
        Self {
            tree,
            files: files.into_iter().collect(),
            honour_range,
            status_code: Arc::new(std::sync::atomic::AtomicU8::new(4)),
            valid_key: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn start(self) -> Fake {
        let Script {
            tree,
            files,
            honour_range,
            status_code,
            valid_key,
        } = self;
        Fake::start(move |req| {
            let path = req.path.as_str();
            if let Some(name) = path.strip_prefix("/dl/") {
                return match files.get(name) {
                    Some(body) => Resp::bytes(req, body, honour_range),
                    None => Resp::raw(404, Vec::new()),
                };
            }
            let wanted = valid_key.lock().unwrap().clone();
            let sent = req
                .headers
                .get("authorization")
                .map(String::as_str)
                .unwrap_or_default();
            if wanted.is_some_and(|key| sent != format!("Bearer {key}")) {
                return Resp::failure("AUTH_BAD_APIKEY", "The apikey is invalid");
            }
            match path {
                "/v4.1/magnet/status" => Resp::data(json!({ "magnets": [{
                    "id": 7, "filename": "Pack", "size": files.values().map(Vec::len).sum::<usize>(),
                    "status": "s", "statusCode": status_code.load(std::sync::atomic::Ordering::SeqCst),
                    "downloaded": 0, "downloadSpeed": 0, "seeders": 0 }] })),
                "/v4/magnet/files" => {
                    Resp::data(json!({ "magnets": [{ "id": "7", "files": tree }] }))
                }
                "/v4/user" => {
                    Resp::data(json!({ "user": { "username": "neo", "isPremium": true } }))
                }
                "/v4/magnet/delete" => Resp::data(json!({ "message": "deleted" })),
                "/v4/link/unlock" => {
                    let link = req.form().get("link").cloned().unwrap_or_default();
                    let name = link.rsplit('/').next().unwrap_or_default().to_string();
                    match files.get(name.as_str()) {
                        Some(body) => Resp::data(json!({
                            "link": format!("http://{}/dl/{name}", host_of(req)),
                            "filesize": body.len(),
                        })),
                        None => Resp::failure("LINK_DOWN", "gone"),
                    }
                }
                _ => Resp::failure("NOPE", "unexpected call"),
            }
        })
    }
}
