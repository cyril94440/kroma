use std::collections::BTreeMap;

use super::notify::COVERAGE_GAP;
use super::{now_ms, Live};
use crate::db::{self, ProviderEvent};
use crate::failover::coverage::{drift, gaps, CoverageRow, Gap};
use crate::model::ProviderId;

#[derive(Debug, Clone, Default)]
pub struct CoverageState {
    pub at: i64,
    pub rows: Option<Vec<CoverageRow>>,
    pub gaps: BTreeMap<String, Gap>,
}

impl Live {
    pub fn coverage(&self) -> CoverageState {
        self.lock().coverage.clone()
    }

    #[cfg(test)]
    pub(crate) fn primary_down_for_test(&self) -> bool {
        self.lock().policy.primary_is_down()
    }

    pub fn trace_lines(&self, limit: usize) -> Vec<String> {
        self.lock().trace.recent(limit)
    }

    pub(crate) fn recheck_coverage(&self) {
        let settings = self.settings();
        if !settings.has_backup() || self.providers.catalogue(ProviderId::Primary).is_empty() || self.providers.catalogue(ProviderId::Backup).is_empty() {
            return;
        }
        let rules = self.rules();
        let table = self.providers.coverage(&rules.favourites, &rules);
        let fresh = {
            let mut inner = self.lock();
            let fresh = drift(inner.coverage.rows.as_deref(), &table);
            inner.coverage = CoverageState { at: now_ms(), gaps: gaps(&table), rows: Some(table) };
            fresh
        };
        for (favourite, gap) in fresh {
            let _ = db::add_provider_event(&self.pool, &ProviderEvent { at: now_ms(), kind: "coverageGap".into(), channel: Some(favourite.clone()), detail: gap.as_str().into(), ..ProviderEvent::default() });
            self.notify_people(COVERAGE_GAP, &favourite, Some(gap.as_str()));
        }
    }
}
