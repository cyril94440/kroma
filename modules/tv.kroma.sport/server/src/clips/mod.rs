mod plan;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub use plan::{
    clip_id, GoalState, AFTER_S, ATTEMPTS_MAX, BEFORE_S, BEFORE_STOPPAGE_S, FALLBACK_AFTER_S,
    FALLBACK_BEFORE_S,
};
pub use plan::{real_times, review, to_cut, GoalReview, Label, Window};
