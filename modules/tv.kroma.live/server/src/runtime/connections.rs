use crate::model::ProviderId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    Probe,
    Trial,
    Capture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    LiveAtThisProvider,
}

impl Refusal {
    pub fn as_str(self) -> &'static str {
        match self {
            Refusal::Busy => "busy",
            Refusal::LiveAtThisProvider => "liveAtThisProvider",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rule<'a> {
    pub single: &'a [ProviderId],
    pub scan_with_live: bool,
}

pub struct Situation {
    pub live: Option<ProviderId>,
    pub tests_at_target: usize,
    pub arriving_at_target: bool,
}

pub fn admit(kind: Use, target: ProviderId, situation: &Situation, rule: &Rule<'_>) -> Result<(), Refusal> {
    if !rule.single.contains(&target) {
        return Ok(());
    }
    let beside_live = kind == Use::Capture && rule.scan_with_live;
    if situation.live == Some(target) && !beside_live {
        return Err(if kind == Use::Trial { Refusal::LiveAtThisProvider } else { Refusal::Busy });
    }
    if situation.tests_at_target > 0 || situation.arriving_at_target {
        return Err(Refusal::Busy);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SINGLE: [ProviderId; 1] = [ProviderId::Backup];

    fn rule(scan_with_live: bool) -> Rule<'static> {
        Rule { single: &SINGLE, scan_with_live }
    }

    fn at(live: Option<ProviderId>) -> Situation {
        Situation { live, tests_at_target: 0, arriving_at_target: false }
    }

    #[test]
    fn the_primary_is_not_limited_and_an_idle_backup_admits_one_test() {
        assert_eq!(admit(Use::Probe, ProviderId::Primary, &at(Some(ProviderId::Primary)), &rule(true)), Ok(()));
        assert_eq!(admit(Use::Probe, ProviderId::Backup, &at(None), &rule(true)), Ok(()));
        let busy = Situation { tests_at_target: 1, ..at(None) };
        assert_eq!(admit(Use::Probe, ProviderId::Backup, &busy, &rule(true)), Err(Refusal::Busy));
    }

    #[test]
    fn a_probe_never_opens_a_second_connection_beside_the_live_at_the_backup() {
        let live = at(Some(ProviderId::Backup));

        assert_eq!(admit(Use::Probe, ProviderId::Backup, &live, &rule(true)), Err(Refusal::Busy));
        assert_eq!(admit(Use::Trial, ProviderId::Backup, &live, &rule(true)), Err(Refusal::LiveAtThisProvider));
    }

    #[test]
    fn a_scan_capture_is_the_one_exception_and_only_one_at_a_time() {
        let live = at(Some(ProviderId::Backup));
        assert_eq!(admit(Use::Capture, ProviderId::Backup, &live, &rule(true)), Ok(()));
        let one_running = Situation { tests_at_target: 1, ..at(Some(ProviderId::Backup)) };
        assert_eq!(admit(Use::Capture, ProviderId::Backup, &one_running, &rule(true)), Err(Refusal::Busy));
        assert_eq!(admit(Use::Capture, ProviderId::Backup, &live, &rule(false)), Err(Refusal::Busy));
    }

    #[test]
    fn nothing_starts_while_the_live_is_arriving_at_the_backup() {
        let arriving = Situation { arriving_at_target: true, ..at(None) };

        assert_eq!(admit(Use::Capture, ProviderId::Backup, &arriving, &rule(true)), Err(Refusal::Busy));
    }

    #[test]
    fn a_relaxed_connection_limit_admits_everything() {
        let live = Situation { tests_at_target: 3, ..at(Some(ProviderId::Backup)) };

        assert_eq!(admit(Use::Trial, ProviderId::Backup, &live, &Rule { single: &[], scan_with_live: false }), Ok(()));
    }
}
