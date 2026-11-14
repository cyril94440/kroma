mod contributors;
mod control;
mod room;

pub use contributors::{Contributors, ExternalTool, PeerContributors, TOOLS};
pub use control::{Candidate, Control, PeerControl, Programme, Slot, Status, Viewer, CONTROL};
#[cfg(test)]
pub use control::{ChannelRef, Snapshot};
pub use room::{PeerRoom, Room, RoomMessage, ROOM};
#[cfg(test)]
pub use room::{KickOutcome, SwitchOutcome};

pub(crate) fn is_unsupported(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("HTTP 404") || text.contains("HTTP 405") || text.contains("no module contributes")
}

#[cfg(test)]
pub mod fakes;
#[cfg(test)]
mod tests;
