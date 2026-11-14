use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const CHUNK: usize = 64 * 1024;

/// One request the server heard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    pub method: String,
    pub target: String,
    pub range: Option<String>,
}

#[derive(Default)]
struct Behaviour {
    redirect: Option<String>,
    announced_total: Option<u64>,
    ignores_ranges: bool,
    status: Option<u16>,
    cut_after: Option<u64>,
}

/// A server on a loopback socket that answers every path with one file, over
/// HTTP/1.1 with persistent connections and `Range`, the way a CDN does. It
/// stops when dropped.
pub struct RangedServer {
    port: u16,
    heard: Arc<Mutex<Vec<Heard>>>,
    behaviour: Arc<Mutex<Behaviour>>,
    stopped: Arc<AtomicBool>,
}

impl RangedServer {
    pub fn serve(file: &Path) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a loopback port");
        listener.set_nonblocking(true).expect("poll the listener");
        let port = listener.local_addr().expect("a bound address").port();
        let server = Self {
            port,
            heard: Arc::default(),
            behaviour: Arc::default(),
            stopped: Arc::default(),
        };
        let (heard, behaviour, stopped) = (server.heard.clone(), server.behaviour.clone(), server.stopped.clone());
        let file = file.to_path_buf();
        std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let Ok((stream, _)) = listener.accept() else {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let (heard, behaviour, file) = (heard.clone(), behaviour.clone(), file.clone());
                std::thread::spawn(move || connection(stream, &file, &heard, &behaviour));
            }
        });
        server
    }

    /// An `http` URL on this server whose query a test can search a log for.
    pub fn url(&self, name: &str) -> String {
        format!("http://127.0.0.1:{}/{name}?token=hunter2", self.port)
    }

    pub fn heard(&self) -> Vec<Heard> {
        self.heard.lock().unwrap().clone()
    }

    /// Answer every request with `status` and no body from now on.
    pub fn fail_with(&self, status: u16) {
        self.behaviour.lock().unwrap().status = Some(status);
    }

    /// Answer every request with a `302` to `location`, as a CDN does when it
    /// moves a file, from now on.
    pub fn redirect_to(&self, location: &str) {
        self.behaviour.lock().unwrap().redirect = Some(location.to_string());
    }

    /// Report `total` as the file's size in every `Content-Range`, as a host
    /// that is serving some other file would.
    pub fn announce_total(&self, total: u64) {
        self.behaviour.lock().unwrap().announced_total = Some(total);
    }

    /// Answer every request with the whole file and a `200`, as a host that
    /// does not support ranges does.
    pub fn ignore_ranges(&self) {
        self.behaviour.lock().unwrap().ignores_ranges = true;
    }

    /// The loopback port this server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Close the connection after `bytes` of a body, as a CDN that drops one does.
    pub fn cut_bodies_after(&self, bytes: u64) {
        self.behaviour.lock().unwrap().cut_after = Some(bytes);
    }
}

impl Drop for RangedServer {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
    }
}

fn connection(stream: TcpStream, file: &Path, heard: &Mutex<Vec<Heard>>, behaviour: &Mutex<Behaviour>) {
    let Ok(reader_half) = stream.try_clone() else { return };
    let mut reader = BufReader::new(reader_half);
    let mut out = stream;
    loop {
        let mut request = String::new();
        if reader.read_line(&mut request).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = request.split_whitespace();
        let (Some(method), Some(target)) = (parts.next(), parts.next()) else { return };
        let (method, target) = (method.to_string(), target.to_string());
        let mut range = None;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line == "\r\n" {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("range") {
                    range = Some(value.trim().to_string());
                }
            }
        }
        heard.lock().unwrap().push(Heard { method: method.clone(), target, range: range.clone() });
        let (status, cut_after, redirect, announced, whole) = {
            let held = behaviour.lock().unwrap();
            (held.status, held.cut_after, held.redirect.clone(), held.announced_total, held.ignores_ranges)
        };
        let range = if whole { None } else { range };
        if let Some(location) = redirect {
            let sent = write!(out, "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
            if sent.is_err() {
                return;
            }
            continue;
        }
        if answer(&mut out, file, &method, range.as_deref(), status, cut_after, announced).is_err() {
            let _ = out.shutdown(Shutdown::Both);
            return;
        }
    }
}

fn answer(
    out: &mut TcpStream,
    file: &Path,
    method: &str,
    range: Option<&str>,
    status: Option<u16>,
    cut_after: Option<u64>,
    announced: Option<u64>,
) -> std::io::Result<()> {
    if let Some(status) = status {
        return write!(out, "HTTP/1.1 {status} Failing\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
    }
    let mut source = std::fs::File::open(file)?;
    let len = source.metadata()?.len();
    let Some((start, end)) = window(range, len) else {
        return write!(out, "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\nContent-Length: 0\r\nConnection: keep-alive\r\n\r\n");
    };
    let length = end - start + 1;
    let head = if range.is_some() {
        format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\n", announced.unwrap_or(len))
    } else {
        "HTTP/1.1 200 OK\r\n".to_string()
    };
    write!(out, "{head}Accept-Ranges: bytes\r\nContent-Type: application/octet-stream\r\nContent-Length: {length}\r\nConnection: keep-alive\r\n\r\n")?;
    if method == "HEAD" {
        return Ok(());
    }
    source.seek(SeekFrom::Start(start))?;
    let mut left = length;
    let mut sent = 0u64;
    let mut buffer = vec![0u8; CHUNK];
    while left > 0 {
        let want = usize::try_from(left.min(CHUNK as u64)).unwrap_or(CHUNK);
        let read = source.read(&mut buffer[..want])?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        out.write_all(&buffer[..read])?;
        sent += read as u64;
        left -= read as u64;
        if cut_after.is_some_and(|cut| sent >= cut) {
            return Err(std::io::ErrorKind::ConnectionAborted.into());
        }
    }
    Ok(())
}

fn window(range: Option<&str>, len: u64) -> Option<(u64, u64)> {
    let Some(spec) = range.and_then(|raw| raw.strip_prefix("bytes=")) else {
        return len.checked_sub(1).map(|end| (0, end));
    };
    let (first, last) = spec.split_once('-')?;
    let end_of_file = len.checked_sub(1)?;
    if first.is_empty() {
        let n: u64 = last.parse().ok()?;
        return Some((len.saturating_sub(n), end_of_file));
    }
    let start: u64 = first.parse().ok()?;
    if start > end_of_file {
        return None;
    }
    let end = if last.is_empty() { end_of_file } else { last.parse::<u64>().ok()?.min(end_of_file) };
    (end >= start).then_some((start, end))
}
