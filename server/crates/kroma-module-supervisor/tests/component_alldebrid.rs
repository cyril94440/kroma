#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::OnceLock;

use kroma_module_host::NetworkAction;
use serde_json::{json, Value};

mod component_support;

use component_support::wasm::build;
use component_support::{Server, wait_until};

const ALLDEBRID: &str = "tv.kroma.engine.alldebrid";
const CLIENT: &str = "/_port/tv.kroma.torrents/client";
const MOVIE_BYTES: usize = 40 * 1024 * 1024;

fn alldebrid_component() -> &'static [u8] {
    static BUILT: OnceLock<Vec<u8>> = OnceLock::new();
    BUILT.get_or_init(|| {
        build(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../modules/tv.kroma.engine.alldebrid/server"),
            "kroma-module-alldebrid",
        )
    })
}

fn movie() -> Vec<u8> {
    (0..MOVIE_BYTES).map(|i| (i % 251) as u8).collect()
}

fn fake_alldebrid() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let body = std::sync::Arc::new(movie());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let body = body.clone();
            std::thread::spawn(move || answer(stream, &body));
        }
    });
    base
}

fn answer(stream: std::net::TcpStream, movie: &[u8]) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (mut length, mut range, mut host) = (0usize, None, String::new());
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        if header == "\r\n" || header.is_empty() {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        } else if let Some(value) = lower.strip_prefix("range: bytes=") {
            let (from, to) = value.trim().split_once('-').unwrap();
            range = Some((from.parse::<usize>().unwrap(), to.parse::<usize>().unwrap()));
        } else if let Some(value) = lower.strip_prefix("host:") {
            host = value.trim().to_string();
        }
    }
    let mut discard = vec![0; length];
    reader.read_exact(&mut discard).unwrap();
    let path = target.split('?').next().unwrap();
    let data = |data: Value| (200, json!({ "status": "success", "data": data }).to_string().into_bytes());
    let (status, body) = match path {
        "/v4.1/magnet/status" => data(json!({ "magnets": [{ "id": 7, "filename": "Pack", "size": MOVIE_BYTES, "statusCode": 4 }] })),
        "/v4/user" => data(json!({ "user": { "username": "neo", "isPremium": true } })),
        "/v4/magnet/files" => data(json!({ "magnets": [{ "id": "7", "files": [
            { "n": "Movie.2020.mkv", "s": MOVIE_BYTES, "l": "https://alldebrid.com/f/Movie.2020.mkv" },
            { "n": "Movie.2020.en.srt", "s": 3, "l": "https://alldebrid.com/f/Movie.2020.en.srt" },
        ] }] })),
        "/v4/link/unlock" => {
            let name = if target.contains("srt") { "Movie.2020.en.srt" } else { "Movie.2020.mkv" };
            let size = if name.ends_with("srt") { 3 } else { MOVIE_BYTES };
            data(json!({ "link": format!("http://{host}/dl/{name}"), "filesize": size }))
        }
        "/dl/Movie.2020.en.srt" => (200, b"abc".to_vec()),
        "/dl/Movie.2020.mkv" => match range {
            Some((from, to)) => (206, movie[from..=to.min(movie.len() - 1)].to_vec()),
            None => (200, movie.to_vec()),
        },
        _ => (404, Vec::new()),
    };
    let mut out = stream;
    let head = format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = out.write_all(head.as_bytes());
    let _ = out.write_all(&body);
}

fn call(server: &Server, endpoint: &kroma_module_supervisor::Endpoint, path: &str, body: &Value) -> Value {
    server
        .as_the_core(ALLDEBRID)
        .post_json(&endpoint.url(path), body)
        .unwrap()
        .json()
        .unwrap()
}

#[test]
fn the_real_alldebrid_module_stubs_a_ready_magnet_and_fills_its_cache_as_a_component() {
    let base = fake_alldebrid();
    let server = Server::new("component-alldebrid", NetworkAction::Direct);
    let endpoint = server.installed_as_shipped(ALLDEBRID, alldebrid_component());
    let client = json!({ "url": base, "password": "key-for-the-test" });
    let status = || call(&server, &endpoint, &format!("{CLIENT}/status"), &json!({ "client": client, "args": { "client_ref": "7" } }));

    let mut done = Value::Null;
    assert!(wait_until(|| {
        done = status();
        done["state"] == "completed"
    }), "{done}");

    let stub = server.data_dir.join("alldebrid/downloads/7/Movie.2020.mkv.kremote");
    assert_eq!(done["progress"], 1.0);
    assert_eq!(done["files"], json!(["Movie.2020.mkv.kremote", "Movie.2020.en.srt"]));
    let saved: Value = serde_json::from_slice(&std::fs::read(&stub).unwrap()).unwrap();
    assert_eq!(saved["ref"], "7:Movie.2020.mkv");
    assert_eq!(std::fs::read(server.data_dir.join("alldebrid/downloads/7/Movie.2020.en.srt")).unwrap(), b"abc");

    let locate = |warm: bool| call(&server, &endpoint, "/_port/source/locate", &json!({ "ref": "7:Movie.2020.mkv", "warm": warm }));
    let first = locate(true);
    assert!(first["Ok"]["url"].is_string(), "{first}");
    let mut landed = Value::Null;
    assert!(wait_until(|| {
        landed = locate(false);
        landed["Ok"]["path"].is_string()
    }), "{landed}");
    let path = landed["Ok"]["path"].as_str().unwrap();
    assert_eq!(std::fs::read(path).unwrap(), movie(), "two ranged chunks were joined byte for byte");
    assert!(
        !server.said.lock().unwrap().iter().any(|line| line.contains("component stopped")),
        "{:?}",
        server.said.lock().unwrap()
    );
    let transfers = server
        .network
        .flows
        .lock()
        .unwrap()
        .iter()
        .filter(|flow| flow.purpose == "debrid-transfer")
        .count();
    assert!(transfers >= 2, "{transfers}");
}
