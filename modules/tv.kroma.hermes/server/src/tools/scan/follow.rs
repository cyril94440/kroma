use std::sync::Arc;

use serde_json::{json, Value};

use super::report;
use super::ScanJob;
use crate::agent::Deps;
use crate::matching::{self, Verdict};
use crate::model::{Model, ScreenReading};
use crate::peers::{is_unsupported, ScanItem, ScanState};
use crate::scans::{Row, RowState, Scan, LIVE_MEMORY_MS};

const POLL_EVERY: u32 = 3;
const MAX_POLL_ERRORS: u32 = 3;

pub struct Reference {
    pub reading: ScreenReading,
    pub at: i64,
    pub source: &'static str,
}

enum Phase {
    Waiting,
    Reference,
    Reading(usize),
}

pub struct Follow {
    job: ScanJob,
    phase: Phase,
    ticks: u32,
    poll_errors: u32,
    state: ScanState,
    rows: Vec<Row>,
    reference: Option<Reference>,
    finished_at: i64,
    pub(super) vision_errors: u32,
    pub(super) late: bool,
    pub(super) frames_unreadable: bool,
}

fn state_of(item: &ScanItem) -> (RowState, String) {
    if item.black {
        return (RowState::Black, "écran noir".into());
    }
    if item.ok {
        return (RowState::Ok, String::new());
    }
    match item.error.as_deref() {
        Some(error) => {
            let reason = report::classify_error(error);
            (if reason.starts_with("non testée") { RowState::NotTested } else { RowState::Error }, reason)
        }
        None => (RowState::NotTested, "non testée".into()),
    }
}

impl Follow {
    pub fn new(job: ScanJob) -> Self {
        Self { job, phase: Phase::Waiting, ticks: 0, poll_errors: 0, state: ScanState::default(), rows: Vec::new(), reference: None, finished_at: 0, vision_errors: 0, late: false, frames_unreadable: false }
    }

    fn wait_for_scan(&mut self, deps: &Deps) -> Option<Value> {
        self.ticks += 1;
        if self.ticks % POLL_EVERY != 1 {
            return None;
        }
        let now = deps.now();
        match deps.control.scan_result() {
            Err(e) => {
                self.poll_errors += 1;
                if self.poll_errors >= MAX_POLL_ERRORS {
                    deps.audit("sources", "scan_result_failed", json!({ "error": format!("{e:#}") }));
                    return Some(json!({ "ok": false, "erreur": format!("résultat du scan illisible : {e:#}"), "scan": "echec" }));
                }
                None
            }
            Ok(state) => {
                self.poll_errors = 0;
                if state.running && now - self.job.started_at < self.job.budget_ms {
                    return None;
                }
                self.late = state.running;
                self.finished_at = now;
                self.rows = state
                    .items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let (row_state, reason) = state_of(item);
                        Row { index, id: item.id.clone(), name: item.name.clone(), number: item.number, state: row_state, reason, reading: None, verdict: None, why: String::new(), target: None, target_why: String::new() }
                    })
                    .collect();
                self.state = state;
                self.phase = if self.job.compare { Phase::Reference } else { Phase::Reading(0) };
                None
            }
        }
    }

    fn read_reference(&mut self, deps: &Deps, model: &Arc<dyn Model>) {
        let now = deps.now();
        let fresh = deps.control.snapshot().ok().and_then(|snap| model.read_screen(&snap.jpeg_base64, "").ok().map(|r| (r, snap.at)));
        if let Some((reading, _)) = fresh.as_ref().filter(|(r, _)| matching::has_teams(r)) {
            if let Some(live) = &self.job.live {
                deps.live_seen.set(live, reading, now);
            }
        }
        let memory = self.job.live.as_deref().and_then(|live| deps.live_seen.teams(live, now, LIVE_MEMORY_MS));
        self.reference = match (fresh, memory) {
            (Some((reading, at)), _) if matching::has_teams(&reading) => Some(Reference { reading, at, source: "direct" }),
            (_, Some(seen)) => Some(Reference { reading: seen.reading, at: seen.at, source: "mémoire" }),
            (Some((reading, at)), None) => Some(Reference { reading, at, source: "direct" }),
            (None, None) => None,
        };
        self.phase = Phase::Reading(0);
    }

    fn read_row(&mut self, deps: &Deps, model: &Arc<dyn Model>, at: usize) {
        let item = &self.state.items[at];
        let Some(path) = item.frame.clone().filter(|_| self.rows[at].state == RowState::Ok && !self.frames_unreadable) else { return };
        match deps.control.scan_frame(&path) {
            Ok(frame) => match model.read_screen(&frame, "") {
                Ok(reading) => self.rows[at].reading = Some(reading),
                Err(e) => {
                    self.vision_errors += 1;
                    deps.audit("vision", "scan_read_failed", json!({ "channel": self.rows[at].name, "error": e.to_string() }));
                }
            },
            Err(e) if is_unsupported(&e) => {
                deps.mark_unsupported("scanFrame", deps.now());
                self.frames_unreadable = true;
            }
            Err(e) => {
                self.vision_errors += 1;
                deps.audit("vision", "scan_frame_failed", json!({ "channel": self.rows[at].name, "error": format!("{e:#}") }));
            }
        }
    }

    fn judge(&mut self, deps: &Deps) {
        let reference = self.reference.as_ref().filter(|r| matching::has_teams(&r.reading) && r.reading.confidence >= matching::CONF_MIN);
        let target = (!self.job.search.is_empty()).then(|| matching::target_from_text(&self.job.search));
        let midpoint = (self.job.started_at + self.finished_at) / 2;
        let captured: Vec<i64> = self.state.items.iter().map(|item| if item.at > 0 { item.at } else { midpoint }).collect();
        for row in self.rows.iter_mut().filter(|r| r.state == RowState::Ok) {
            let Some(reading) = row.reading.clone() else { continue };
            if let Some(reference) = reference {
                let dt = (captured[row.index] - reference.at) as f64 / 1000.0;
                let (verdict, why) = matching::decide(&reference.reading, &reading, dt);
                row.verdict = Some(verdict);
                row.why = why;
            }
            if let Some(target) = &target {
                let (verdict, why) = matching::match_target(target, &reading);
                row.target = Some(verdict);
                row.target_why = why;
            }
        }
        deps.audit("decisions", "scan_verdicts", json!({ "scan": self.job.id, "reference": self.reference.as_ref().map(|r| matching::resume_fr(Some(&r.reading))), "rows": self.rows.iter().map(|r| json!({ "channel": r.name, "state": r.state.label(), "reading": matching::resume_fr(r.reading.as_ref()), "verdict": r.verdict.map(Verdict::label), "why": r.why, "target": r.target.map(Verdict::label), "target_why": r.target_why })).collect::<Vec<_>>() }));
    }

    fn finish(&mut self, deps: &Deps) -> Value {
        self.judge(deps);
        let usable = self.reference.as_ref().is_some_and(|r| matching::has_teams(&r.reading) && r.reading.confidence >= matching::CONF_MIN);
        let groups = crate::scans::build_groups(&self.rows);
        let scan_id = deps.scans.next_id();
        let reference = self.reference.as_ref();
        let ref_reading = reference.map(|r| &r.reading);
        let scan = Scan {
            id: scan_id,
            at: deps.now(),
            provider: self.job.provider.clone(),
            query: if self.job.query.is_empty() { "références".into() } else { self.job.query.clone() },
            compared: self.job.compare && usable,
            search: self.job.search.clone(),
            live: self.job.live.clone(),
            by: self.job.by.clone(),
            ref_teams: ref_reading.map(crate::scans::team_names).unwrap_or_default(),
            ref_at: reference.map(|r| r.at),
            rows: self.rows.clone(),
            groups,
        };
        let out = report::build(&self.job, &scan, reference, usable, self);
        deps.scans.add(scan);
        deps.audit("sources", "scan", json!({ "scan": scan_id, "provider": self.job.provider, "total": self.rows.len(), "late": self.late, "vision_errors": self.vision_errors, "frames_unreadable": self.frames_unreadable }));
        out
    }

    pub fn step(&mut self, deps: &Deps, model: &Arc<dyn Model>) -> Option<Value> {
        match self.phase {
            Phase::Waiting => self.wait_for_scan(deps),
            Phase::Reference => {
                self.read_reference(deps, model);
                None
            }
            Phase::Reading(at) if at >= self.rows.len() => Some(self.finish(deps)),
            Phase::Reading(at) => {
                if deps.now() - self.job.started_at > self.job.budget_ms {
                    self.late = true;
                    return Some(self.finish(deps));
                }
                self.read_row(deps, model, at);
                self.phase = Phase::Reading(at + 1);
                None
            }
        }
    }
}
