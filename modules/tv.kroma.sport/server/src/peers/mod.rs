mod live;
mod room;

#[cfg(test)]
pub mod fakes;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub use live::{Benchmarked, BufferInfo, Candidate, CutClip, ScanReport, Slot};
pub use live::{ChannelRef, Live, LiveStatus, PeerLive, Programme, Viewer, CONTROL};
pub use room::{PeerRoom, Room, SwitchOutcome, ROOM};

pub(crate) fn is_unsupported(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("HTTP 404") || text.contains("HTTP 405") || text.contains("no module")
}
