mod contributors;
mod control;
mod ops;
mod room;

pub use contributors::{Contributors, ExternalTool, PeerContributors, TOOLS};
pub use control::{Candidate, Control, PeerControl, Programme, Slot, Status, Viewer, CONTROL};
#[cfg(test)]
pub use control::{ChannelRef, EventInfo, Snapshot};
pub use ops::{BenchmarkStart, History, HistoryPoint, Probe, ProviderSwitch, Restart, ScanItem, ScanState};
pub use room::{PeerRoom, Room, RoomMessage, SwitchAsk, ROOM};
#[cfg(test)]
pub use room::{KickOutcome, SwitchOutcome};

pub(crate) fn is_unsupported(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("HTTP 404") || text.contains("HTTP 405") || text.contains("no module contributes")
}

#[cfg(test)]
pub mod fake_control;
#[cfg(test)]
pub mod fakes;
#[cfg(test)]
mod tests;
