mod competitions;
mod decide;
mod flat;
mod image;
mod names;
mod scoring;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub use competitions::is_special;
pub use competitions::{slug_competition, slug_espn};
#[cfg(test)]
pub use decide::{candidates, confirms, Decision};
pub use decide::{contradicts, decide, merge, Verdict, POST_WINDOW_S};
pub use flat::{flat_matches, FlatMatch};
#[cfg(test)]
pub use image::score_compatible;
pub use names::name_designates;
#[cfg(test)]
pub use names::{team_forms, Forms, TeamNames};
pub use scoring::DetectProgramme;
