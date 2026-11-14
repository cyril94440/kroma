use anyhow::{bail, Context, Result};
use kroma_module_sdk::primitives::remote_file::{self, RemoteFile};

use crate::api::Magnet;
use crate::cache::{within, Availability, Cache};
use crate::files::{kind_of, Kind, Planned};
use crate::resolve::{is_unusable, plan_with};
use crate::status::base;
use crate::types::{TorrentState, TorrentStatus};
use crate::AllDebrid;

pub const SOURCE: &str = "alldebrid";
const MAX_SUBTITLE_BYTES: u64 = 5 * 1024 * 1024;
const FETCHING_SIDECARS: f64 = 0.99;

pub fn reference_of(magnet: &str, rel: &str) -> String {
    format!("{magnet}:{rel}")
}

pub fn parse_reference(reference: &str) -> Option<(&str, &str)> {
    let (magnet, rel) = reference.split_once(':')?;
    (crate::ensure_magnet_id(magnet).is_ok() && !rel.is_empty()).then_some((magnet, rel))
}

impl AllDebrid {
    /// What `status` reports for a magnet AllDebrid has finished: one
    /// `.kremote` stub per video and the small subtitle files beside them,
    /// under the magnet's own folder. `completed` is only reported once every
    /// stub exists and no sidecar is still on its way.
    pub fn ready_status(&self, client_ref: &str, magnet: &Magnet) -> Result<TorrentStatus> {
        let plan = match plan_with(&self.api, client_ref) {
            Ok(plan) => plan,
            Err(error) if is_unusable(&error) => {
                let mut status = base(client_ref, magnet);
                status.state = TorrentState::Error;
                status.error = Some(format!("{error:#}"));
                return Ok(status);
            }
            Err(error) => return Err(error),
        };
        let folder = self.stubs_folder(client_ref);
        let mut written = Vec::new();
        let mut waiting = false;
        let mut videos = 0;
        for file in &plan {
            match kind_of(&file.rel) {
                Kind::Video => {
                    written.push(write_stub(&folder, client_ref, file)?);
                    videos += 1;
                }
                Kind::Subtitle if file.size <= MAX_SUBTITLE_BYTES => {
                    match Cache::ensure_at(&self.api, &within(&folder, &file.rel), file) {
                        Availability::Cached(_) => written.push(file.rel.clone()),
                        Availability::Fetching { .. } => waiting = true,
                        Availability::Failed(_) | Availability::Declined(_) => {}
                    }
                }
                Kind::Subtitle | Kind::Other => {}
            }
        }
        let mut status = base(client_ref, magnet);
        if videos == 0 {
            status.state = TorrentState::Error;
            status.error = Some("AllDebrid has no video file in this magnet".into());
            return Ok(status);
        }
        if waiting {
            status.state = TorrentState::Downloading;
            status.progress = FETCHING_SIDECARS;
            return Ok(status);
        }
        status.state = TorrentState::Completed;
        status.progress = 1.0;
        status.save_path = Some(folder.to_string_lossy().into_owned());
        status.files = written;
        Ok(status)
    }
}

fn write_stub(folder: &std::path::Path, magnet: &str, file: &Planned) -> Result<String> {
    let stub_rel = format!("{}.{}", file.rel, remote_file::EXTENSION);
    let path = within(folder, &stub_rel);
    if !path.starts_with(folder) {
        bail!("a stub would land outside its folder");
    }
    let wanted = RemoteFile {
        source: SOURCE.into(),
        reference: reference_of(magnet, &file.rel),
        name: file.rel.rsplit('/').next().unwrap_or(&file.rel).to_string(),
        size: file.size,
    };
    if remote_file::read(&path).ok().as_ref() != Some(&wanted) {
        remote_file::write(&path, &wanted)
            .with_context(|| format!("save the stub for {}", file.rel))?;
    }
    Ok(stub_rel)
}
