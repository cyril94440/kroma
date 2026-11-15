use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const USER: &str = "TESTUSER";
pub const PASS: &str = "TESTPASS42";

pub struct FakePortal {
    pub port: u16,
    pub broken: Arc<AtomicBool>,
    pub connections: Arc<AtomicUsize>,
    pub streams_open: Arc<AtomicUsize>,
    pub max_open: Arc<AtomicUsize>,
    pub streams: Arc<std::sync::Mutex<Vec<String>>>,
}

fn channels_json() -> String {
    serde_json::json!([
        {"name": "FR: TF1 FHD", "stream_id": 1, "epg_channel_id": "TF1.fr", "category_id": "1", "is_adult": "0"},
        {"name": "VIP: CANAL+ ᴿᴬᵂ", "stream_id": 2, "epg_channel_id": "canalplus.fr", "category_id": "1"},
        {"name": "FR: M6 HD", "stream_id": 3, "epg_channel_id": "m6.fr", "category_id": "2"},
        {"name": "FR: ARTE 4K", "stream_id": 4, "epg_channel_id": "arte.fr", "category_id": "2"},
        {"name": "XXX NIGHT", "stream_id": 5, "category_id": "9"},
        {"name": "FR: OFFAIR HD", "stream_id": 6, "epg_channel_id": "", "category_id": "2"}
    ])
    .to_string()
}

pub fn guide_xml(now_s: i64) -> String {
    let stamp = |at: i64| {
        let days = at.div_euclid(86_400);
        let secs = at.rem_euclid(86_400);
        let (y, m, d) = civil(days);
        format!("{y:04}{m:02}{d:02}{:02}{:02}{:02} +0000", secs / 3600, secs % 3600 / 60, secs % 60)
    };
    let programme = |channel: &str, start: i64, stop: i64, title: &str, desc: &str| {
        format!("<programme start=\"{}\" stop=\"{}\" channel=\"{channel}\"><title>{title}</title><desc>{desc}</desc></programme>", stamp(start), stamp(stop))
    };
    format!(
        "<?xml version=\"1.0\"?><tv>{}{}{}{}</tv>",
        programme("TF1.fr", now_s - 1800, now_s + 1800, "Journal", "Les infos &amp; la meteo"),
        programme("TF1.fr", now_s + 1800, now_s + 5400, "Film du soir", "Un film"),
        programme("canalplus.fr", now_s - 600, now_s + 3000, "PSG Liverpool", "Ligue des champions"),
        programme("m6.fr", now_s - 100, now_s + 900, "Capital", "Magazine"),
    )
}

fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn respond(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn lavfi_for(stream_id: &str) -> Vec<String> {
    let video = match stream_id {
        "3" => "smptebars=size=320x180:rate=25",
        "6" => "color=c=black:size=320x180:rate=25",
        _ => "testsrc2=size=320x180:rate=25",
    };
    ["-v", "error", "-re", "-f", "lavfi", "-i", video, "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-c:v", "libx264", "-preset", "ultrafast", "-tune", "zerolatency", "-g", "25", "-keyint_min", "25", "-sc_threshold", "0", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "64k", "-f", "mpegts", "pipe:1"].map(String::from).to_vec()
}

fn serve_stream(mut stream: TcpStream, id: &str, state: &FakePortal) {
    let (broken, open) = (&state.broken, &state.streams_open);
    let head = "HTTP/1.1 200 OK\r\nContent-Type: video/mp2t\r\nConnection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let Ok(mut child) = Command::new("ffmpeg").args(lavfi_for(id)).stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null()).spawn() else { return };
    let mut out = child.stdout.take().unwrap();
    let now_open = open.fetch_add(1, Ordering::SeqCst) + 1;
    state.max_open.fetch_max(now_open, Ordering::SeqCst);
    let (sender, receiver) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = out.read(&mut buf) {
            if n == 0 || sender.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let _ = stream.set_nonblocking(true);
    while !broken.load(Ordering::SeqCst) {
        let mut probe = [0u8; 1];
        match stream.peek(&mut probe) {
            Ok(0) => break,
            Err(error) if error.kind() != std::io::ErrorKind::WouldBlock => break,
            _ => {}
        }
        match receiver.recv_timeout(Duration::from_millis(5)) {
            Ok(chunk) => {
                let _ = stream.set_nonblocking(false);
                let sent = stream.write_all(&chunk).is_ok();
                let _ = stream.set_nonblocking(true);
                if !sent {
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
    }
    open.fetch_sub(1, Ordering::SeqCst);
    let _ = child.kill();
    let _ = child.wait();
}

fn handle(mut stream: TcpStream, state: &FakePortal) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
    }
    state.connections.fetch_add(1, Ordering::SeqCst);
    let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    if path.starts_with("/player_api.php") {
        let body = if path.contains("get_live_streams") {
            channels_json()
        } else if path.contains("get_live_categories") {
            r#"[{"category_id":"1","category_name":"General"},{"category_id":"2","category_name":"Other"}]"#.into()
        } else {
            r#"{"user_info":{"status":"Active"}}"#.into()
        };
        return respond(&mut stream, "200 OK", "application/json", body.as_bytes());
    }
    if path.starts_with("/xmltv.php") {
        use std::io::Write as _;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        encoder.write_all(guide_xml(now).as_bytes()).unwrap();
        return respond(&mut stream, "200 OK", "application/xml", &encoder.finish().unwrap());
    }
    let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    if parts.len() == 3 && parts[0] == USER && parts[1] == PASS {
        let id = parts[2].trim_end_matches(".ts");
        state.streams.lock().unwrap().push(id.to_string());
        if state.broken.load(Ordering::SeqCst) {
            return respond(&mut stream, "503 Service Unavailable", "text/plain", b"down");
        }
        return serve_stream(stream, id, state);
    }
    respond(&mut stream, "404 Not Found", "text/plain", b"nope");
}

impl FakePortal {
    pub fn start() -> FakePortal {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let portal = FakePortal {
            port: listener.local_addr().unwrap().port(),
            broken: Arc::new(AtomicBool::new(false)),
            connections: Arc::new(AtomicUsize::new(0)),
            streams_open: Arc::new(AtomicUsize::new(0)),
            max_open: Arc::new(AtomicUsize::new(0)),
            streams: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        let shared = FakePortal { port: portal.port, broken: portal.broken.clone(), connections: portal.connections.clone(), streams_open: portal.streams_open.clone(), max_open: portal.max_open.clone(), streams: portal.streams.clone() };
        let shared = Arc::new(shared);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = shared.clone();
                std::thread::spawn(move || handle(stream, &state));
            }
        });
        portal
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/get.php?username={USER}&password={PASS}&type=m3u_plus", self.port)
    }
}
