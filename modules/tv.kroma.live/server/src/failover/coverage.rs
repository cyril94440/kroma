use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CoverageRow {
    pub favourite: String,
    pub primary: Option<String>,
    pub backup: Vec<String>,
    pub guide: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Gap {
    AbsentFromBackup,
    NotFound,
    NoGuide,
}

impl Gap {
    pub fn as_str(self) -> &'static str {
        match self {
            Gap::AbsentFromBackup => "absentFromBackup",
            Gap::NotFound => "notFound",
            Gap::NoGuide => "noGuide",
        }
    }
}

pub fn gaps(table: &[CoverageRow]) -> BTreeMap<String, Gap> {
    let mut out = BTreeMap::new();
    for row in table {
        if row.backup.is_empty() {
            out.insert(row.favourite.clone(), if row.primary.is_some() { Gap::AbsentFromBackup } else { Gap::NotFound });
        } else if !row.guide {
            out.insert(row.favourite.clone(), Gap::NoGuide);
        }
    }
    out
}

pub fn drift(old: Option<&[CoverageRow]>, new: &[CoverageRow]) -> BTreeMap<String, Gap> {
    let before = old.map(gaps).unwrap_or_default();
    gaps(new).into_iter().filter(|(favourite, gap)| before.get(favourite) != Some(gap)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(favourite: &str, primary: Option<&str>, backup: &[&str], guide: bool) -> CoverageRow {
        CoverageRow { favourite: favourite.into(), primary: primary.map(str::to_string), backup: backup.iter().map(|b| b.to_string()).collect(), guide }
    }

    #[test]
    fn a_favourite_the_backup_would_not_save_is_a_gap_with_its_reason() {
        let table = [row("TF1", Some("FR: TF1 FHD"), &["|FR| TF1 FHD"], true), row("ARTE", Some("FR: ARTE FHD"), &[], true), row("X", None, &[], false), row("M6", Some("FR: M6 FHD"), &["|FR| M6 FHD"], false)];

        let found = gaps(&table);

        assert_eq!(found.len(), 3);
        assert_eq!(found["ARTE"], Gap::AbsentFromBackup);
        assert_eq!(found["X"], Gap::NotFound);
        assert_eq!(found["M6"], Gap::NoGuide);
    }

    #[test]
    fn the_first_computation_reports_every_gap_and_later_ones_only_the_new() {
        let first = [row("TF1", Some("a"), &["b"], true), row("ARTE", Some("c"), &[], true)];
        let second = [row("TF1", Some("a"), &[], true), row("ARTE", Some("c"), &[], true)];

        assert_eq!(drift(None, &first).keys().collect::<Vec<_>>(), ["ARTE"]);
        assert!(drift(Some(&first), &first).is_empty());
        assert_eq!(drift(Some(&first), &second).keys().collect::<Vec<_>>(), ["TF1"]);
    }
}
