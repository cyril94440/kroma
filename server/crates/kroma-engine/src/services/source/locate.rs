use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::warn;

use kroma_module_host::call_raw;

use crate::state::{Contributions, OfficialModules};

use super::{Refusal, SourceError};

/// The bare point the core calls to find where a stubbed file's bytes are.
pub const POINT: &str = "source";

const LOCATE_PATH: &str = "source/locate";

#[derive(Debug, Serialize)]
struct Ask<'a> {
    #[serde(rename = "ref")]
    reference: &'a str,
    warm: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Answer {
    path: Option<String>,
    url: Option<String>,
}

/// What a source answered for one file, and which module said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    pub module_id: String,
    pub path: Option<String>,
    pub url: Option<String>,
}

/// The one official module that answers `source` for a title. A module the
/// official registry did not ship is not asked, and two official modules
/// naming the same instance are both refused rather than one winning.
pub(super) fn answerer(
    contributions: &Contributions,
    official: &OfficialModules,
    source: &str,
) -> Result<kroma_module_host::Contribution, SourceError> {
    let trusted = official();
    let mut named = contributions(POINT)
        .into_iter()
        .filter(|found| found.instance.as_deref() == Some(source))
        .filter(|found| trusted.contains(&found.module_id));
    let first = named.next().ok_or(SourceError::NoModule)?;
    if named.next().is_some() {
        warn!(
            source,
            "more than one official module answers this source; none is asked"
        );
        return Err(SourceError::Refused(Refusal::DuplicateSource));
    }
    Ok(first)
}

pub(super) fn locate(
    contributions: &Contributions,
    official: &OfficialModules,
    source: &str,
    reference: &str,
    warm: bool,
) -> Result<Located, SourceError> {
    let found = answerer(contributions, official, source)?;
    let reply: Result<Answer, String> = call_raw(
        &found.resolver(),
        LOCATE_PATH,
        &json!(Ask { reference, warm }),
    )
    .map_err(|_| SourceError::Unreachable)?;
    let answer = reply.map_err(SourceError::Gone)?;
    Ok(Located {
        module_id: found.module_id,
        path: answer.path.filter(|path| !path.is_empty()),
        url: answer.url.filter(|url| !url.is_empty()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_is_a_ref_and_a_warm_flag_and_nothing_else() {
        let body = serde_json::to_value(Ask {
            reference: "7:Show/a.mkv",
            warm: true,
        })
        .unwrap();

        assert_eq!(body, json!({ "ref": "7:Show/a.mkv", "warm": true }));
    }

    #[test]
    fn an_answer_may_carry_a_path_a_url_or_more_than_it_was_asked_for() {
        let by_path: Result<Answer, String> = serde_json::from_value(
            json!({ "Ok": { "path": "/d/a.mkv", "url": null, "size": 9, "later": 1 } }),
        )
        .unwrap();
        let by_url: Result<Answer, String> =
            serde_json::from_value(json!({ "Ok": { "url": "https://x.example/a" } })).unwrap();
        let refused: Result<Answer, String> =
            serde_json::from_value(json!({ "Err": "gone" })).unwrap();

        assert_eq!(by_path.unwrap().path.as_deref(), Some("/d/a.mkv"));
        assert_eq!(by_url.unwrap().url.as_deref(), Some("https://x.example/a"));
        assert_eq!(refused.unwrap_err(), "gone");
    }
}
