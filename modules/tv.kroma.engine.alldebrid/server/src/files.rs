//! AllDebrid's nested file tree, flattened to relative paths that are safe to
//! create under a download folder whatever the tree names its entries.

use std::collections::HashSet;

use std::fmt;

use anyhow::Result;

use crate::api::Node;

const MAX_DEPTH: usize = 16;
const MAX_FILES: usize = 5_000;
const MAX_NAME_BYTES: usize = 200;
const MAX_EXTENSION_BYTES: usize = 16;

#[derive(Debug)]
pub struct Unusable(String);

impl fmt::Display for Unusable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unusable {}

pub(crate) fn unusable(reason: impl Into<String>) -> anyhow::Error {
    Unusable(reason.into()).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Video,
    Subtitle,
    Other,
}

const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "webvtt"];

pub fn kind_of(rel: &str) -> Kind {
    let Some((_, extension)) = rel.rsplit_once('.') else {
        return Kind::Other;
    };
    let extension = extension.to_ascii_lowercase();
    if kroma_module_sdk::primitives::video::VIDEO_EXTENSIONS.contains(&extension.as_str()) {
        Kind::Video
    } else if SUBTITLE_EXTENSIONS.contains(&extension.as_str()) {
        Kind::Subtitle
    } else {
        Kind::Other
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub rel: String,
    pub size: u64,
    pub link: String,
}

pub fn flatten(tree: &[Node]) -> Result<Vec<Planned>> {
    let mut out = Vec::new();
    let mut taken = HashSet::new();
    walk(tree, &mut Vec::new(), &mut out, &mut taken)?;
    if out.is_empty() {
        return Err(unusable("the magnet holds no file with a download link"));
    }
    Ok(out)
}

fn walk(
    nodes: &[Node],
    parents: &mut Vec<String>,
    out: &mut Vec<Planned>,
    taken: &mut HashSet<String>,
) -> Result<()> {
    if parents.len() >= MAX_DEPTH {
        return Err(unusable("the magnet nests its folders too deeply"));
    }
    for node in nodes {
        let name = component(&node.n);
        if let Some(children) = &node.e {
            parents.push(name);
            walk(children, parents, out, taken)?;
            parents.pop();
            continue;
        }
        let Some(link) = node.l.as_deref().filter(|link| !link.is_empty()) else {
            continue;
        };
        if out.len() >= MAX_FILES {
            return Err(unusable(format!("the magnet holds more than {MAX_FILES} files")));
        }
        out.push(Planned {
            rel: unique(parents, &name, taken),
            size: node.s.unwrap_or(0),
            link: link.to_string(),
        });
    }
    Ok(())
}

fn unique(parents: &[String], name: &str, taken: &mut HashSet<String>) -> String {
    let join = |name: &str| {
        parents
            .iter()
            .map(String::as_str)
            .chain([name])
            .collect::<Vec<_>>()
            .join("/")
    };
    let mut rel = join(name);
    let mut copy = 1;
    while !taken.insert(rel.to_lowercase()) {
        copy += 1;
        rel = join(&numbered(name, copy));
    }
    rel
}

fn numbered(name: &str, copy: u32) -> String {
    let tail = format!(" ({copy})");
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= MAX_EXTENSION_BYTES => {
            let room = MAX_NAME_BYTES.saturating_sub(ext.len() + 1 + tail.len());
            format!("{}{tail}.{ext}", cut(stem, room).trim_end())
        }
        _ => {
            let room = MAX_NAME_BYTES.saturating_sub(tail.len());
            format!("{}{tail}", cut(name, room).trim_end())
        }
    }
}

pub fn component(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c == '/' || c == '\\' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed.chars().all(|c| c == '.') {
        return "_".to_string();
    }
    truncated(trimmed)
}

fn truncated(name: &str) -> String {
    if name.len() <= MAX_NAME_BYTES {
        return name.to_string();
    }
    match name.rsplit_once('.') {
        Some((stem, extension))
            if !stem.is_empty()
                && extension.len() <= MAX_EXTENSION_BYTES
                && !extension.contains(char::is_whitespace) =>
        {
            let room = MAX_NAME_BYTES - extension.len() - 1;
            format!("{}.{extension}", cut(stem, room).trim_end())
        }
        _ => cut(name, MAX_NAME_BYTES).trim_end().to_string(),
    }
}

fn cut(text: &str, bytes: usize) -> &str {
    let mut end = bytes.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests;
