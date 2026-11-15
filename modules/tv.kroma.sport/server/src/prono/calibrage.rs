use serde::{Deserialize, Serialize};

pub const MIN_DELAY_S: f64 = 20.0;
pub const MEASURES_MIN: usize = 3;
pub const MEASURES_N: usize = 7;
pub const DELAY_MAX_S: f64 = 180.0;
pub const SPREAD_MAX_S: f64 = 30.0;
pub const PERSIST_MAX_S: f64 = 24.0 * 3600.0;

pub type DirectKey = Vec<String>;

pub fn measured_delay(capture_s: f64, chrono_s: f64, change_s: f64, clock_s: f64) -> f64 {
    (capture_s - chrono_s) - (change_s - (clock_s - 60.0))
}

pub fn valid_measure(delay: f64) -> bool {
    (0.0..=DELAY_MAX_S).contains(&delay)
}

pub fn median(values: &[f64]) -> Option<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    (n > 0).then(|| {
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.0
        }
    })
}

pub fn robust_median(values: &[f64]) -> Option<f64> {
    let ok: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| valid_measure(*v))
        .collect();
    let m = median(&ok)?;
    median(
        &ok.iter()
            .copied()
            .filter(|x| (x - m).abs() <= SPREAD_MAX_S)
            .collect::<Vec<_>>(),
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Persisted {
    pub median_s: f64,
    pub at: f64,
    #[serde(default)]
    pub key: Option<DirectKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chrono {
    pub ts_s: f64,
    pub chrono_s: f64,
    pub period: i64,
    #[serde(default)]
    pub regime: Option<DirectKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub delai_s: f64,
    pub source: &'static str,
    pub mesures: usize,
    pub mediane_s: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Calibrage {
    pub match_id: Option<String>,
    pub key: Option<DirectKey>,
    pub measures: Vec<f64>,
    pub rejects: (i8, u32),
    pub persisted: Option<Persisted>,
}

impl Calibrage {
    pub fn with_persisted(persisted: Option<Persisted>) -> Self {
        Self {
            persisted: persisted.filter(|p| valid_measure(p.median_s)),
            ..Self::default()
        }
    }

    pub fn for_match(&mut self, match_id: Option<&str>) {
        if match_id != self.match_id.as_deref() {
            self.match_id = match_id.map(str::to_string);
            self.measures.clear();
            self.key = None;
            self.rejects = (0, 0);
        }
    }

    pub fn add(&mut self, delay: f64, now: f64, key: Option<&DirectKey>) -> bool {
        let Some(key) = key else { return false };
        if !valid_measure(delay) {
            return false;
        }
        if self.key.as_ref() != Some(key) {
            self.key = Some(key.clone());
            self.measures.clear();
            self.rejects = (0, 0);
        }
        if let (true, Some(m)) = (
            self.measures.len() >= MEASURES_MIN,
            robust_median(&self.measures),
        ) {
            if (delay - m).abs() > SPREAD_MAX_S {
                let side = if delay > m { 1 } else { -1 };
                self.rejects = (
                    side,
                    if self.rejects.0 == side {
                        self.rejects.1 + 1
                    } else {
                        1
                    },
                );
                if self.rejects.1 < 2 {
                    return false;
                }
                self.measures.clear();
                self.persisted = None;
            }
        }
        self.rejects = (0, 0);
        self.measures.push(delay);
        let excess = self.measures.len().saturating_sub(MEASURES_N);
        self.measures.drain(..excess);
        if self.measures.len() >= MEASURES_MIN {
            let rounded = (robust_median(&self.measures).unwrap_or(0.0) * 10.0).round() / 10.0;
            self.persisted = Some(Persisted {
                median_s: rounded,
                at: now,
                key: Some(key.clone()),
            });
        }
        true
    }

    pub fn delay(
        &self,
        default_s: f64,
        now: f64,
        key: Option<&DirectKey>,
        margin_s: f64,
    ) -> (f64, &'static str) {
        if self.measures.len() >= MEASURES_MIN {
            let v = MIN_DELAY_S.max(robust_median(&self.measures).unwrap_or(0.0) + margin_s);
            return if key.is_some() && key == self.key.as_ref() {
                (v, "mesure")
            } else {
                (default_s.max(v), "mesure_autre_direct")
            };
        }
        let base = self.delay_without_measures(default_s, now, key, margin_s);
        if let (Some(max), true) = (
            self.measures.iter().copied().reduce(f64::max),
            key.is_some() && key == self.key.as_ref(),
        ) {
            let v = MIN_DELAY_S.max(max + margin_s);
            if v > base.0 {
                return (v, "mesures_partielles");
            }
        }
        base
    }

    fn delay_without_measures(
        &self,
        default_s: f64,
        now: f64,
        key: Option<&DirectKey>,
        margin_s: f64,
    ) -> (f64, &'static str) {
        match &self.persisted {
            Some(p) if (0.0..=PERSIST_MAX_S).contains(&(now - p.at)) => {
                let v = MIN_DELAY_S.max(p.median_s + margin_s);
                if key.is_some() && key == p.key.as_ref() {
                    (v, "persistee")
                } else {
                    (default_s.max(v), "persistee_autre_direct")
                }
            }
            _ => (default_s, "defaut"),
        }
    }

    pub fn diagnostic(
        &self,
        default_s: f64,
        now: f64,
        key: Option<&DirectKey>,
        margin_s: f64,
    ) -> Diagnostic {
        let (delay, source) = self.delay(default_s, now, key, margin_s);
        Diagnostic {
            delai_s: (delay * 10.0).round() / 10.0,
            source,
            mesures: self.measures.len(),
            mediane_s: robust_median(&self.measures).map(|m| (m * 10.0).round() / 10.0),
        }
    }
}
