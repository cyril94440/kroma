use std::fs::{File, OpenOptions};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use kroma_module_sdk::http::Fetch;

use super::disk;
use crate::api::Api;
use crate::files::Planned;

const PURPOSE: &str = "debrid-transfer";
pub const CHUNK_BYTES: u64 = if cfg!(test) {
    64 * 1024
} else {
    32 * 1024 * 1024
};
const CHUNK_SECS: u32 = 120;
const MAX_RELOCKS: u8 = 2;
const MAX_FAILURES: u8 = 3;

pub struct Step {
    pub moved: u64,
    pub finished: bool,
}

impl Step {
    fn moved(moved: u64) -> Self {
        Self {
            moved,
            finished: false,
        }
    }
}

pub struct FileJob {
    api: Api,
    link: String,
    direct: Option<String>,
    dest: PathBuf,
    size: u64,
    relocks: u8,
    failures: u8,
}

impl FileJob {
    pub fn new(api: Api, dest: PathBuf, file: &Planned) -> Self {
        Self {
            api,
            link: file.link.clone(),
            direct: None,
            dest,
            size: file.size,
            relocks: 0,
            failures: 0,
        }
    }

    pub fn is_complete(&self) -> bool {
        disk::is_complete(&self.dest, self.size)
    }

    pub fn advance(&mut self) -> Result<Step> {
        if self.direct.is_none() {
            self.unlock()?;
        }
        let part = disk::part_of(&self.dest);
        let have = disk::len(&part).unwrap_or(0);
        if have > self.size {
            let _ = std::fs::remove_file(&part);
            bail!(
                "a partial file was larger than the {} bytes AllDebrid reports",
                self.size
            );
        }
        if have == self.size {
            return self.finish(0);
        }
        self.fetch_next(have)
    }

    fn unlock(&mut self) -> Result<()> {
        let unlocked = self.api.unlock(&self.link)?;
        if self.size == 0 {
            self.size = unlocked
                .filesize
                .ok_or_else(|| anyhow!("AllDebrid did not report this file's size"))?;
        }
        crate::transfer_hosts::warn_if_unlisted(&unlocked.link);
        self.direct = Some(unlocked.link);
        Ok(())
    }

    fn fetch_next(&mut self, have: u64) -> Result<Step> {
        let part = disk::part_of(&self.dest);
        let chunk = disk::chunk_of(&self.dest);
        if let Some(parent) = self.dest.parent() {
            std::fs::create_dir_all(parent).context("create the download folder")?;
        }
        let _ = std::fs::remove_file(&chunk);
        let end = (have + CHUNK_BYTES).min(self.size) - 1;
        let direct = self.direct.clone().unwrap_or_default();
        let sent = Fetch::new()
            .purpose(PURPOSE)
            .max_time(CHUNK_SECS)
            .max_bytes(self.size)
            .header("range", format!("bytes={have}-{end}"))
            .download(&direct, &chunk);
        let response = match sent {
            Ok(response) => response,
            Err(error) => return self.stumbled(&chunk, error),
        };
        let got = disk::len(&chunk).unwrap_or(0);
        let asked = end - have + 1;
        let outcome = match response.status {
            206 => match content_range(response.header("content-range")) {
                Some(range)
                    if range.start == have
                        && got > 0
                        && range.end == have + got - 1
                        && got <= asked
                        && range.total.is_none_or(|total| total == self.size) =>
                {
                    append(&chunk, &part).map(|()| got)
                }
                _ => Err(anyhow!(
                    "the download server answered a range other than bytes {have}-{end}"
                )),
            },
            200 if got == self.size => replace(&chunk, &part).map(|()| got.saturating_sub(have)),
            401 | 403 | 404 | 410 if self.relocks < MAX_RELOCKS => {
                self.relocks += 1;
                self.direct = None;
                Ok(0)
            }
            200 => Err(anyhow!("the download server sent {got} unexpected bytes")),
            other => Err(anyhow!("the download server answered HTTP {other}")),
        };
        let _ = std::fs::remove_file(&chunk);
        let moved = outcome?;
        self.failures = 0;
        if moved > 0 {
            self.relocks = 0;
        }
        if disk::len(&part) == Some(self.size) {
            return self.finish(moved);
        }
        Ok(Step::moved(moved))
    }

    fn stumbled(&mut self, chunk: &std::path::Path, error: anyhow::Error) -> Result<Step> {
        let _ = std::fs::remove_file(chunk);
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            return Err(error);
        }
        Ok(Step::moved(0))
    }

    fn finish(&self, moved: u64) -> Result<Step> {
        let part = disk::part_of(&self.dest);
        if let Some(parent) = self.dest.parent() {
            std::fs::create_dir_all(parent).context("create the download folder")?;
        }
        if disk::len(&part).is_none() {
            File::create(&part).context("create the empty file")?;
        }
        std::fs::rename(&part, &self.dest).context("move the finished file into place")?;
        Ok(Step {
            moved,
            finished: true,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ContentRange {
    start: u64,
    end: u64,
    total: Option<u64>,
}

fn content_range(header: Option<&str>) -> Option<ContentRange> {
    let spec = header?.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = spec.split_once('/')?;
    let (start, end) = span.trim().split_once('-')?;
    Some(ContentRange {
        start: start.parse().ok()?,
        end: end.parse().ok()?,
        total: total.trim().parse().ok(),
    })
}

fn append(chunk: &std::path::Path, part: &std::path::Path) -> Result<()> {
    let mut from = File::open(chunk).context("open the chunk")?;
    let mut to = OpenOptions::new()
        .create(true)
        .append(true)
        .open(part)
        .context("open the partial file")?;
    std::io::copy(&mut from, &mut to).context("append the chunk")?;
    Ok(())
}

fn replace(chunk: &std::path::Path, part: &std::path::Path) -> Result<()> {
    std::fs::rename(chunk, part).context("keep the whole file")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_content_range_is_read_with_or_without_a_known_total() {
        assert_eq!(
            content_range(Some("bytes 0-99/1000")),
            Some(ContentRange {
                start: 0,
                end: 99,
                total: Some(1000)
            })
        );
        assert_eq!(
            content_range(Some("bytes 5-9/*")),
            Some(ContentRange {
                start: 5,
                end: 9,
                total: None
            })
        );
    }

    #[test]
    fn anything_else_is_no_range() {
        assert_eq!(content_range(None), None);
        assert_eq!(content_range(Some("bytes */1000")), None);
        assert_eq!(content_range(Some("items 0-9/10")), None);
    }
}
