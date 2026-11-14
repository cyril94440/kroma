use anyhow::{bail, Result};
use kroma_module_sdk::http::Fetch;

use crate::db;
use crate::epg_parse::{parse_xmltv, read_body};
use crate::model::ProviderId;
use crate::portal::redact;
use crate::runtime::{now_ms, Live};

const GUIDE_BYTES: u64 = 256 * 1024 * 1024;
const GUIDE_SECONDS: u32 = 180;

impl Live {
    fn guide_sources(&self) -> Vec<(String, String, &'static str)> {
        let settings = self.settings();
        let mut sources = Vec::new();
        if !settings.epg_url.is_empty() {
            sources.push((settings.epg_url.clone(), settings.user_agent.clone(), "override"));
        }
        for provider in [ProviderId::Primary, ProviderId::Backup] {
            if let Some(portal) = settings.portal(provider) {
                sources.push((portal.xmltv_url(), settings.user_agent_of(provider).to_string(), provider.as_str()));
            }
        }
        sources
    }

    pub fn refresh_epg(&self) -> Result<usize> {
        let sources = self.guide_sources();
        if sources.is_empty() {
            bail!("no guide source is configured");
        }
        let mut last = String::new();
        for (url, agent, label) in sources {
            match self.load_guide(&url, &agent) {
                Ok(rows) => {
                    let count = rows.len();
                    db::replace_programmes(self.pool(), &rows, label, now_ms())?;
                    return Ok(count);
                }
                Err(error) => last = redact(&format!("{error:#}")),
            }
        }
        let _ = db::note_epg_failure(self.pool(), &last);
        bail!("{last}")
    }

    fn load_guide(&self, url: &str, agent: &str) -> Result<Vec<db::StoredProgramme>> {
        let response = Fetch::new().purpose("provider").max_time(GUIDE_SECONDS).max_bytes(GUIDE_BYTES).header("user-agent", agent).header("accept-encoding", "gzip").get(url)?.ensure_ok()?;
        let xml = read_body(&response.body)?;
        parse_xmltv(&xml, now_ms() / 1000)
    }
}
