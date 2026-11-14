//! Files an official module publishes to signed-in clients: `GET
//! /api/module-files/<module>/<path>` serves `<data>/<dataDir>/public/<path>`
//! for a module that declares `publish: ["files"]`.
//!
//! A path token is the module's business: a module that must gate who fetches
//! puts an unguessable folder name in the path it hands only to those people.

use std::fs::File as StdFile;
use std::net::SocketAddr;
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{ConnectInfo, Extension, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use kroma_media::playlist::with_credential;
use kroma_media::stream::stream_open;
use kroma_module_supervisor::Supervisor;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::api::error::json_error;
use crate::api::media_ticket::{MediaViewer, TicketQuery};
use crate::api::permission::require;
use crate::api::stream::byte_sink;
use crate::model::Permission;
use crate::state::SharedState;

const MAX_PLAYLIST_BYTES: u64 = 1024 * 1024;
const MAX_PATH_BYTES: usize = 512;
const SEGMENT_CACHE: &str = "private, max-age=30";
const PLAYLIST_MIME: &str = "application/vnd.apple.mpegurl";

pub fn routes() -> Router<SharedState> {
    Router::new().route("/module-files/{module}/{*path}", get(serve))
}

async fn serve(
    State(state): State<SharedState>,
    MediaViewer(viewer): MediaViewer,
    Extension(supervisor): Extension<Arc<Supervisor>>,
    Path((module, path)): Path<(String, String)>,
    Query(ticket): Query<TicketQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Response, Response> {
    require(&viewer, Permission::Playback)?;
    let not_found = || json_error(StatusCode::NOT_FOUND, "file not found");
    if !kroma_engine::modules::module_enabled(&state.settings, &module) {
        return Err(not_found());
    }
    let relative = safe_relative(&path).ok_or_else(not_found)?;
    let root = supervisor.public_root_of(&module).ok_or_else(not_found)?;
    let opened = tokio::task::spawn_blocking(move || open_under(&root, &relative))
        .await
        .map_err(|_| not_found())?
        .ok_or_else(not_found)?;
    let extension = opened.path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mime = content_type(extension);
    if extension.eq_ignore_ascii_case("m3u8") {
        return playlist(opened.file, &ticket).await.ok_or_else(not_found);
    }
    let sink = byte_sink(&state, &headers, &addr);
    let mut response = stream_open(File::from_std(opened.file), mime, &headers, sink).await;
    mark(&mut response, SEGMENT_CACHE);
    Ok(response)
}

async fn playlist(file: StdFile, ticket: &TicketQuery) -> Option<Response> {
    let mut text = String::new();
    File::from_std(file)
        .take(MAX_PLAYLIST_BYTES)
        .read_to_string(&mut text)
        .await
        .ok()?;
    let body = with_credential(&text, &ticket.credential());
    let mut response = Response::new(Body::from(body));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(PLAYLIST_MIME));
    mark(&mut response, "no-store");
    Some(response)
}

fn mark(response: &mut Response, cache: &'static str) {
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
}

fn content_type(extension: &str) -> &'static str {
    match extension.to_ascii_lowercase().as_str() {
        "m3u8" => PLAYLIST_MIME,
        "m4s" | "mp4" | "m4v" | "cmfv" | "cmfa" => "video/mp4",
        "ts" => "video/mp2t",
        "aac" => "audio/aac",
        "vtt" => "text/vtt; charset=utf-8",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn safe_relative(path: &str) -> Option<PathBuf> {
    if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains(['\\', '\0']) {
        return None;
    }
    let mut relative = PathBuf::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment.starts_with('.') {
            return None;
        }
        relative.push(segment);
    }
    relative
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
        .then_some(relative)
}

struct Opened {
    file: StdFile,
    path: PathBuf,
}

fn open_under(root: &FsPath, relative: &FsPath) -> Option<Opened> {
    let root = root.canonicalize().ok()?;
    let resolved = root.join(relative).canonicalize().ok()?;
    if !resolved.starts_with(&root) {
        return None;
    }
    let file = open_without_following(&resolved).ok()?;
    file.metadata().ok()?.is_file().then_some(Opened {
        file,
        path: resolved,
    })
}

#[cfg(unix)]
fn open_without_following(path: &FsPath) -> std::io::Result<StdFile> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_without_following(path: &FsPath) -> std::io::Result<StdFile> {
    StdFile::open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_plain_segments_or_nothing() {
        assert_eq!(
            safe_relative("abc/seg1.m4s"),
            Some(PathBuf::from("abc/seg1.m4s"))
        );
        for bad in ["", "../x", "a/../x", "/etc/passwd", "a//b", ".hidden", "a/.b", "a\\b", "a/\0"] {
            assert_eq!(safe_relative(bad), None, "{bad:?}");
        }
        assert_eq!(safe_relative(&"a".repeat(MAX_PATH_BYTES + 1)), None);
    }

    #[test]
    fn a_container_is_served_as_what_a_player_expects() {
        assert_eq!(content_type("m3u8"), "application/vnd.apple.mpegurl");
        assert_eq!(content_type("M4S"), "video/mp4");
        assert_eq!(content_type("mp4"), "video/mp4");
        assert_eq!(content_type("ts"), "video/mp2t");
        assert_eq!(content_type("vtt"), "text/vtt; charset=utf-8");
        assert_eq!(content_type("jpg"), "image/jpeg");
        assert_eq!(content_type("png"), "image/png");
        assert_eq!(content_type("exe"), "application/octet-stream");
    }
}
