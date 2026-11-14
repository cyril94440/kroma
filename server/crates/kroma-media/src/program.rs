//! One HLS program served out of the engine, whatever it reads: the mode the
//! client's declared decoders settle it on, its media playlist, and the files
//! ffmpeg writes for it.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use kroma_module_wire::{AudioStream, VideoStream};
use serde::Deserialize;

use crate::hls::{Asked, HlsEngine, Refusal, StreamMode, ToneMap};
use crate::input::MediaInput;
use crate::json_error::json_error;
use crate::stream::metered_body;
use crate::{playlist, ByteMeter, Credential};

/// What a client says its decoder takes, read off the query of every request for
/// a program. `copy` names the audio codecs it can decode or pass through, so
/// the server can refuse to stream-copy one the device would play silent;
/// `video` does the same for the picture, which would otherwise play black.
/// Absent means no declared capability (the requested mode is trusted); present
/// but empty means the client decodes none, so any copied stream is transcoded.
/// `maxw` / `maxh` are the largest picture its decoder accepts, both axes, and
/// absent leaves the source's own size standing. `hdr` names the dynamic-range
/// formats its display shows, from `hdr10`, `hdr10plus`, `hlg` and `dv`: absent
/// means undeclared (the requested mode is trusted), present but empty means
/// SDR only, so an HDR source is tone-mapped down to a picture of at most 1080p
/// where this host can map it.
///
/// A Dolby Vision profile 5 stream has no picture a non-Dolby decoder can read
/// as it is. Where the host can tone-map, a client that declared `hdr` without
/// `dv` gets the mapped re-encode. Where it cannot (no libplacebo,
/// `KROMA_TONEMAP=off`, a build without the filters), and for a client that
/// declared no `hdr` at all, the stream is copied tagged as plain HEVC with no
/// Dolby Vision box, as it was before the box existed: it opens everywhere and
/// shows the base layer's own colours. Only a client that declared `dv` gets
/// `dvh1` and the box, which is what the Apple HLS authoring rules ask of
/// profile 5 and what no other decoder can open.
#[derive(Debug, Default, Deserialize)]
pub struct ClientDecoders {
    pub copy: Option<String>,
    pub video: Option<String>,
    pub hdr: Option<String>,
    pub maxw: Option<u32>,
    pub maxh: Option<u32>,
}

/// One session of a program: the mode it is produced in, the second it starts
/// from, and the audio track muxed into it. Each is its own ffmpeg with its own
/// segment URLs.
#[derive(Clone, Copy, Debug)]
pub struct Session {
    pub mode: StreamMode,
    pub anchor: u64,
    pub audio: u32,
}

/// What the engine needs to know about one program to pick its mode and start
/// its ffmpeg. `id` names its sessions, so two programs must never share one.
pub struct Program<'a> {
    pub id: &'a str,
    pub video: Option<&'a VideoStream>,
    pub audio_tracks: &'a [AudioStream],
    pub duration_ms: Option<u64>,
}

impl Program<'_> {
    fn frame(&self) -> Option<(u32, u32)> {
        self.video.and_then(|v| v.width.zip(v.height))
    }

    fn track(&self, audio: u32) -> Option<&AudioStream> {
        self.audio_tracks.iter().find(|t| t.index == audio)
    }

    /// The mode whose picture and sound will actually play on the client, for the
    /// `audio`-th track. Both axes are resolved before a caller compares it with
    /// the asked mode, so one answer settles them together.
    pub fn effective_mode(
        &self,
        mode: StreamMode,
        audio: u32,
        declared: &ClientDecoders,
    ) -> StreamMode {
        let selected = self.track(audio);
        let (codec, channels) = (
            selected.map(|t| t.codec.as_str()),
            selected.and_then(|t| t.channels),
        );
        let tone_map = self.video.and_then(ToneMap::for_video);
        mode.for_client_track(codec, channels, declared.copy.as_deref())
            .for_client_video(
                self.video.map(|v| v.codec.as_str()),
                self.video.and_then(|v| v.bit_depth),
                declared.video.as_deref(),
            )
            .for_client_hdr(
                self.video,
                declared.hdr.as_deref(),
                declared.hdr.is_some() && tone_map.is_some(),
            )
            .within_tone_map_box(tone_map.is_some())
            .for_client_frame(self.frame(), declared.maxw.zip(declared.maxh))
            .for_tone_map_cost(self.frame(), tone_map.map(ToneMap::route))
            .for_copied_picture(self.video, codec, channels, declared.copy.as_deref())
    }

    /// The media playlist of `session`, starting it over the file when none
    /// runs, with every child URI carrying `credential`. `opener` names the
    /// player asking, so its own earlier anchor of this program is taken back
    /// rather than left running beside the one it just asked for. The
    /// [`Refusal`] says which answer the caller owes the client: a host already
    /// at its re-encode ceiling is worth asking again, a missing playlist is not.
    pub async fn master<M: ByteMeter>(
        &self,
        engine: &HlsEngine,
        input: &MediaInput,
        session: Session,
        opener: Option<&str>,
        credential: &Credential,
        meter: M,
    ) -> Result<Response, Refusal> {
        let Session {
            mode,
            anchor,
            audio,
        } = session;
        let (body, start) = engine
            .master(Asked {
                item_id: self.id,
                input,
                opener,
                audio,
                mode,
                anchor,
                source: self.frame(),
                video: self.video,
                track: self.track(audio),
            })
            .await?;
        let body = playlist::with_credential(&body, credential);
        let mut resp = playlist_response(body, meter);
        // `X-Hls-Start` is the real start (the keyframe at-or-before the anchor, where
        // `-noaccurate_seek` begins); the client needs it to align clock and subtitles.
        insert_seconds(&mut resp, "X-Hls-Start", start);
        // `X-Media-Duration` is the true total length in seconds: without it a
        // client missing `durationMs` would size its slider to the growing EVENT
        // playlist's live edge.
        let duration_ms = match self.duration_ms {
            Some(d) => Some(d),
            None => engine.input_duration_ms(input).await,
        };
        if let Some(secs) = duration_ms
            .map(|ms| ms as f64 / 1000.0)
            .filter(|s| *s > 0.0)
        {
            insert_seconds(&mut resp, "X-Media-Duration", secs);
        }
        Ok(resp)
    }
}

/// A child file (init or media segment) of `session` of program `id`. A
/// not-yet-produced segment is polled for until ffmpeg flushes it.
pub async fn file<M: ByteMeter>(
    engine: &HlsEngine,
    id: &str,
    session: Session,
    name: &str,
    meter: M,
) -> Response {
    let immutable = !name.ends_with(".m3u8");
    let Session {
        mode,
        anchor,
        audio,
    } = session;
    match engine.file(id, mode, anchor, audio, name).await {
        Some((bytes, ct)) => Response::builder()
            .header(header::CONTENT_TYPE, ct)
            .header(header::CONTENT_LENGTH, bytes.len())
            // Each anchor's URLs are unique, so segment bytes never change; event
            // playlists grow.
            .header(
                header::CACHE_CONTROL,
                if immutable {
                    "public, max-age=31536000, immutable"
                } else {
                    "no-store"
                },
            )
            .body(metered_body(std::io::Cursor::new(bytes), meter))
            .unwrap(),
        // A segment that has not been produced yet 404s the same way one that was
        // pruned does, so the miss must never be cached: the URL becomes valid.
        None => {
            let mut resp = json_error(
                StatusCode::NOT_FOUND,
                "segment not found (session expired?)",
            );
            resp.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            resp
        }
    }
}

fn insert_seconds(resp: &mut Response, name: &'static str, secs: f64) {
    if let Ok(v) = HeaderValue::from_str(&format!("{secs:.3}")) {
        resp.headers_mut().insert(name, v);
    }
}

fn playlist_response<M: ByteMeter>(body: String, meter: M) -> Response {
    let bytes = body.into_bytes();
    Response::builder()
        .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .body(metered_body(std::io::Cursor::new(bytes), meter))
        .unwrap()
}

#[cfg(test)]
mod tests;
