mod compute;
mod espn;

#[cfg(test)]
mod tests;

pub use compute::{norm_club, provisional, LiveGame, StandingRow};
pub use espn::{competition_code, from_espn};
