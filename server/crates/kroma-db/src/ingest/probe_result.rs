//! Persisting one probe result, and the representative columns it moves.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

use kroma_domain::{AudioStream, HdrFormat, SubtitleTrack, VideoStream};

use crate::Pool;

/// What one probe learned about a file, as [`set_file_probe`] writes it. A set
/// `unreadable` arrives with no streams: the container never opened.
#[derive(Debug, Default)]
pub struct FileProbe<'a> {
    pub duration_ms: Option<u64>,
    pub video: Option<&'a VideoStream>,
    pub audio: Option<&'a AudioStream>,
    pub audio_tracks: &'a [AudioStream],
    pub subtitles: &'a [SubtitleTrack],
    pub unreadable: Option<&'a str>,
}

/// Persists one file's probe result, then recomputes the owning item's representative columns.
pub fn set_file_probe(pool: &Pool, file_id: &str, probe: &FileProbe) -> Result<()> {
    let conn = pool.get()?;
    let subs = serde_json::to_string(probe.subtitles).unwrap_or_else(|_| "[]".into());
    let a_tracks = serde_json::to_string(probe.audio_tracks).unwrap_or_else(|_| "[]".into());
    let (video, audio) = (probe.video, probe.audio);
    let color = video.and_then(|v| v.color.as_ref());
    conn.execute(
        "UPDATE files SET probed=1, duration_ms=?2, \
            v_codec=?3, v_width=?4, v_height=?5, v_hdr=?6, v_bit_depth=?7, \
            a_codec=?8, a_channels=?9, a_language=?10, subtitles=?11, audio_tracks=?12, \
            unreadable=?13, v_hdr_format=?14, v_dv_profile=?15, \
            v_color_primaries=?16, v_color_transfer=?17, v_color_matrix=?18, \
            v_dv_level=?19, v_dv_compat=?20, v_dv_el=?21, probe_schema=?22 \
         WHERE id = ?1",
        params![
            file_id,
            probe.duration_ms.map(|d| d as i64),
            video.map(|v| v.codec.clone()),
            video.and_then(|v| v.width),
            video.and_then(|v| v.height),
            video.map(|v| v.hdr as i64),
            video.and_then(|v| v.bit_depth),
            audio.map(|a| a.codec.clone()),
            audio.and_then(|a| a.channels),
            audio.and_then(|a| a.language.clone()),
            subs,
            a_tracks,
            probe.unreadable,
            video.and_then(|v| v.hdr_format).map(HdrFormat::as_str),
            video.and_then(|v| v.dolby_vision_profile),
            color.and_then(|c| c.primaries.clone()),
            color.and_then(|c| c.transfer.clone()),
            color.and_then(|c| c.matrix.clone()),
            video.and_then(|v| v.dolby_vision_level),
            video.and_then(|v| v.dolby_vision_compatibility),
            video.and_then(|v| v.dolby_vision_enhancement_layer.then_some(1)),
            super::probe::PROBE_SCHEMA,
        ],
    )?;

    let item_id: Option<String> = conn
        .query_row(
            "SELECT item_id FROM files WHERE id = ?1",
            params![file_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(item_id) = item_id {
        recompute_item_representative(&conn, &item_id)?;
    }
    Ok(())
}

fn recompute_item_representative(conn: &Connection, item_id: &str) -> Result<()> {
    let best: Option<(String, String, Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT abs_path, container, rel_path, duration_ms FROM files \
             WHERE item_id = ?1 AND probed = 1 AND unreadable IS NULL \
             ORDER BY v_width DESC NULLS LAST, id LIMIT 1",
            params![item_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .optional()?;

    if let Some((abs, container, rel, duration)) = best {
        conn.execute(
            "UPDATE items SET container=?2, abs_path=?3, rel_path=?4, duration_ms=?5 WHERE id=?1",
            params![item_id, container, abs, rel, duration],
        )?;
    }
    Ok(())
}

pub(super) fn recompute_all_representatives(pool: &Pool) -> Result<()> {
    pool.get()?.execute(
        "UPDATE items AS i \
            SET container = b.container, abs_path = b.abs_path, \
                rel_path = b.rel_path, duration_ms = b.duration_ms \
           FROM (SELECT item_id, abs_path, container, rel_path, duration_ms, \
                        ROW_NUMBER() OVER (PARTITION BY item_id \
                            ORDER BY v_width DESC NULLS LAST, id) AS rank \
                   FROM files WHERE probed = 1 AND unreadable IS NULL) AS b \
          WHERE b.item_id = i.id AND b.rank = 1 \
            AND (i.container IS NOT b.container OR i.abs_path IS NOT b.abs_path \
              OR i.rel_path IS NOT b.rel_path OR i.duration_ms IS NOT b.duration_ms)",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
