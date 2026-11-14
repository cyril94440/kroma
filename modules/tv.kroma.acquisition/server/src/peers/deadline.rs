use std::cell::Cell;
use std::time::Instant;

const WIDEST_CALL_SECS: u32 = 30;

thread_local! {
    static DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Runs `work` with every call to a peer made on this thread capped to the time
/// left before `deadline`, so a search or a grab cannot outlast the budget its
/// caller gave.
pub fn within<T>(deadline: Instant, work: impl FnOnce() -> T) -> T {
    let before = DEADLINE.with(|slot| slot.replace(Some(deadline)));
    let done = work();
    DEADLINE.with(|slot| slot.set(before));
    done
}

pub(crate) fn call_secs() -> u32 {
    DEADLINE.with(Cell::get).map_or(WIDEST_CALL_SECS, |deadline| {
        let left = deadline.saturating_duration_since(Instant::now());
        (left.as_secs_f64().ceil() as u32).clamp(1, WIDEST_CALL_SECS)
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn calls_are_capped_to_the_time_left_and_never_past_the_widest() {
        assert_eq!(call_secs(), WIDEST_CALL_SECS);
        let secs = within(Instant::now() + Duration::from_secs(5), call_secs);

        assert!((4..=5).contains(&secs), "{secs}");
        assert_eq!(call_secs(), WIDEST_CALL_SECS);
    }

    #[test]
    fn a_spent_deadline_still_leaves_one_second_to_fail_in() {
        let secs = within(Instant::now(), call_secs);

        assert_eq!(secs, 1);
    }
}
