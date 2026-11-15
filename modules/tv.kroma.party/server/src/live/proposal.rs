use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProposalBody {
    pub id: String,
    pub kind: String,
    pub requested_by: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProposalEvent {
    pub proposal: ProposalBody,
    pub resolved: bool,
    pub accepted: bool,
    pub default: String,
}
