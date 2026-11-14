//! On-demand HLS delivery.
//!
//! Direct-play (raw byte range, [`crate::stream`]) is the default; this
//! covers what a browser can't direct-play: MKV→fMP4 repackaging, audio it
//! can't decode (AC3/EAC3/DTS → AAC), video it can't decode (HEVC → H.264), and
//! language switching. A thin wrapper over a [`session`] registry: one
//! continuous ffmpeg per (item, mode, anchor) produces a complete-program HLS
//! master. The anchor is part of both the session key and the URL path, so two
//! anchors never write each other's segment names; a re-anchor into a second one
//! of them has already produced is answered out of it ([`covered`]) rather than
//! sharing a session.

mod admission;
mod args;
mod covered;
mod cut;
mod ffmpeg;
mod fit;
mod hwaccel;
mod key;
mod live;
mod mode;
mod naming;
mod progress;
mod reclaim;
mod session;
mod software;
mod tonemap;
mod window;

pub use admission::Refusal;
pub use hwaccel::Detection;
pub use live::Transcode;
pub use mode::{AudioMode, Rung, StreamMode, VideoMode, SURROUND_MIN_CHANNELS};
pub use tonemap::{filters as tonemap_filters, Filters, Route, ToneMap, ToneSource, SDR_TAGS};
pub(crate) use fit::declares;
pub(crate) use hwaccel::tone_map_filter;

use std::path::PathBuf;
#[cfg(test)]
use std::path::Path;
use std::sync::Arc;

use kroma_module_wire::{AudioStream, VideoStream};

use crate::input::MediaInput;

use key::session_key;
use session::Sessions;

/// What a master request asks for: the program, the stream wanted out of it, and
/// an opaque id for the player asking.
///
/// `anchor` is the start in seconds (input `-ss`, 0 = start) and `audio` the
/// audio-relative track index. `source` is the file's own frame size where the
/// catalog knows it, which is what a re-encode has to decode before it can
/// encode anything. Two requests carrying the same `opener` are the same player,
/// which is how a seek is told from a second viewer on the same title; `None`
/// is a caller that presented no credential, and matches nobody. `video` and
/// `track` are what the catalog read of the stream and of the audio track asked
/// for, which decide the flags a copy needs and what a re-encode keeps.
pub struct Asked<'a> {
    pub item_id: &'a str,
    pub input: &'a MediaInput,
    pub opener: Option<&'a str>,
    pub audio: u32,
    pub mode: StreamMode,
    pub anchor: u64,
    pub source: Option<(u32, u32)>,
    pub video: Option<&'a VideoStream>,
    pub track: Option<&'a AudioStream>,
}

/// Probes what this host can re-encode with, once, at boot and off the async
/// runtime: the encoder devices, the tone-mapping filters and which tone-map
/// routes a device runs, and what the ffmpeg build lists. They run side by side;
/// a request that arrives before they finish is answered conservatively (the
/// CPU, no tone-map) and never waits on them.
pub fn prime_hwaccel() {
    std::thread::scope(|scope| {
        scope.spawn(crate::capabilities::prime);
        let filters = scope.spawn(tonemap::detect_filters);
        hwaccel::prime();
        let filters = filters.join().unwrap_or(Filters::NONE);
        tonemap::prime(hwaccel::detect(), filters);
    });
    tracing::info!(support = ?tonemap::support(), "tone-mapping settled");
}

pub struct HlsEngine {
    sessions: Arc<Sessions>,
    durations: std::sync::Mutex<std::collections::HashMap<String, Option<u64>>>,
}

impl HlsEngine {
    /// `root` is the directory the engine writes its sessions in, and it is
    /// emptied on construction. `max_concurrent` hard-caps live sessions;
    /// `cache_budget` is the on-disk byte budget that trims idle / superseded
    /// sessions (0 = unlimited).
    pub fn new(root: PathBuf, max_concurrent: usize, cache_budget: u64) -> Self {
        HlsEngine {
            sessions: Arc::new(Sessions::new(root, max_concurrent, cache_budget)),
            durations: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// True media duration (ms) of the input, ffprobed once (duration-only,
    /// header read) and kept. Lets the player show the real length when the
    /// catalog row was never probed: otherwise the growing EVENT playlist's
    /// duration is all the client can see (it reads the live edge as the total).
    ///
    /// Only a file on this machine is kept: a network URL is short-lived and a
    /// bearer, so it is read each time and never stored.
    pub async fn input_duration_ms(&self, input: &MediaInput) -> Option<u64> {
        let key = input.local().map(|path| path.to_string_lossy().into_owned());
        if let Some(known) = key
            .as_ref()
            .and_then(|key| self.durations.lock().unwrap().get(key).copied())
        {
            return known;
        }
        let held = input.clone();
        let read = tokio::task::spawn_blocking(move || crate::probe::probe_duration_ms(&held))
            .await
            .ok()
            .flatten();
        if let Some(key) = key {
            self.durations.lock().unwrap().insert(key, read);
        }
        read
    }

    pub fn spawn_reaper(&self) {
        self.sessions.spawn_reaper();
    }

    pub fn cache_bytes(&self) -> u64 {
        self.sessions.bytes()
    }

    /// Retune the on-disk cache byte budget at runtime (0 = unlimited).
    pub fn set_cache_budget(&self, bytes: u64) {
        self.sessions.set_budget(bytes);
    }

    /// The media playlist for [`Asked`]. Returns the playlist text + the REAL
    /// stream start (s) - the keyframe at-or-before the anchor - for the
    /// client's `baseSec`.
    pub async fn master(&self, asked: Asked<'_>) -> Result<(String, f64), Refusal> {
        let key = session_key(asked.item_id, asked.mode, asked.anchor, asked.audio);
        let (bytes, start) = self.sessions.master(&key, &asked).await?;
        let playlist = String::from_utf8(bytes).map_err(|_| Refusal::Unavailable)?;
        Ok((playlist, start))
    }

    /// Every remux running right now, newest first: what it is producing, on
    /// what silicon, and whether it is keeping ahead of the player.
    pub async fn live(&self) -> Vec<Transcode> {
        self.sessions.live().await
    }

    /// The pipeline this host settled on for re-encodes, and the sentence that
    /// explains it. A device that is present, listed and unusable looks exactly
    /// like no device at all, and the reason is the only thing that separates them.
    pub fn hardware() -> &'static Detection {
        hwaccel::detected()
    }

    /// A child file (init or segment) of the `(mode, anchor, audio)` session.
    pub async fn file(
        &self,
        item_id: &str,
        mode: StreamMode,
        anchor: u64,
        audio: u32,
        name: &str,
    ) -> Option<(Vec<u8>, &'static str)> {
        let key = session_key(item_id, mode, anchor, audio);
        self.sessions.file(&key, name).await
    }
}

#[cfg(test)]
mod tests;
