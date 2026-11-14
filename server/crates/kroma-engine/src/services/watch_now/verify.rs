//! Whether a file, read back, is what its release said it was.

const LOWEST_FRACTION_OF_RUNTIME: f64 = 0.6;
const HIGHEST_FRACTION_OF_RUNTIME: f64 = 1.8;
const TIERS: [u32; 4] = [480, 720, 1080, 2160];

/// What probing the file found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    pub unreadable: Option<String>,
    pub duration_ms: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// What the file was promised to be: how long TMDB says the title runs, how
/// many episodes the file's own name says it holds, whether it is one part of a
/// multi-part movie, and the resolution its release name claims. Either is
/// absent when nobody knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Promise {
    pub runtime_min: Option<u32>,
    pub claimed_height: Option<u32>,
    pub episodes: u32,
    pub multi_part: bool,
}

impl Default for Promise {
    fn default() -> Self {
        Self {
            runtime_min: None,
            claimed_height: None,
            episodes: 1,
            multi_part: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Genuine,
    Fake(String),
}

/// Judges the probed file. The duration band is wide because cuts differ, a
/// file named for a run of episodes is held to that many runtimes, a part of a
/// multi-part movie to none, and the resolution may fall one tier below its
/// claim, because a name rounds.
pub fn judge(facts: &Facts, promise: &Promise) -> Verdict {
    if let Some(reason) = &facts.unreadable {
        return Verdict::Fake(format!("the file cannot be read ({reason})"));
    }
    if facts.height.is_none() && facts.width.is_none() {
        return Verdict::Fake("the file holds no video".into());
    }
    let timed = (!promise.multi_part).then_some(()).and(promise.runtime_min);
    if let (Some(runtime), Some(duration_ms)) = (timed, facts.duration_ms) {
        let minutes = duration_ms as f64 / 60_000.0;
        let expected = f64::from(runtime) * f64::from(promise.episodes.max(1));
        if minutes < expected * LOWEST_FRACTION_OF_RUNTIME
            || minutes > expected * HIGHEST_FRACTION_OF_RUNTIME
        {
            return Verdict::Fake(format!(
                "the file runs {minutes:.0} minutes and the title runs {expected:.0}"
            ));
        }
    }
    if let Some(claimed) = promise.claimed_height {
        let (found, wanted) = (tier(facts), tier_of(claimed));
        if found + 1 < wanted {
            return Verdict::Fake(format!(
                "the file is {}p and its release claims {claimed}p",
                TIERS[found]
            ));
        }
    }
    Verdict::Genuine
}

fn tier(facts: &Facts) -> usize {
    let by_height = facts.height.map_or(0, tier_of);
    let by_width = facts.width.map_or(0, tier_of_width);
    by_height.max(by_width)
}

fn tier_of(height: u32) -> usize {
    match height {
        0..=540 => 0,
        541..=800 => 1,
        801..=1500 => 2,
        _ => 3,
    }
}

fn tier_of_width(width: u32) -> usize {
    match width {
        0..=800 => 0,
        801..=1400 => 1,
        1401..=2800 => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests;
