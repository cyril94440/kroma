use crate::db::{self, Pool, StoredProgramme};
use crate::model::Programme;

pub fn to_programme(row: StoredProgramme, channel_name: &str) -> Programme {
    Programme {
        channel_id: row.epg_id,
        channel_name: channel_name.to_string(),
        title: row.title,
        description: row.description,
        start: row.start * 1000,
        end: row.stop * 1000,
    }
}

pub fn now_next(pool: &Pool, epg_id: Option<&str>, channel_name: &str, now_ms: i64) -> (Option<Programme>, Option<Programme>) {
    let Some(epg_id) = epg_id else { return (None, None) };
    let now_s = now_ms / 1000;
    let rows = db::programmes_of(pool, epg_id, now_s - 1, now_s + 14 * 3600).unwrap_or_default();
    let current = rows.iter().find(|r| r.start <= now_s && now_s < r.stop).cloned();
    let upcoming = rows.iter().find(|r| r.start > now_s).cloned();
    (current.map(|r| to_programme(r, channel_name)), upcoming.map(|r| to_programme(r, channel_name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{replace_programmes, test_pool};

    fn row(start: i64, stop: i64, title: &str) -> StoredProgramme {
        StoredProgramme { epg_id: "tf1.fr".into(), start, stop, title: title.into(), hay: title.into(), description: String::new() }
    }

    #[test]
    fn now_and_next_come_from_the_channels_guide_in_milliseconds() {
        let pool = test_pool();
        replace_programmes(&pool, &[row(0, 100, "Before"), row(100, 200, "Now"), row(200, 300, "Next"), row(300, 400, "Later")], "p", 1).unwrap();

        let (now, next) = now_next(&pool, Some("TF1.fr"), "TF1", 150_000);

        assert_eq!((now.as_ref().map(|p| p.title.as_str()), now.as_ref().map(|p| (p.start, p.end))), (Some("Now"), Some((100_000, 200_000))));
        assert_eq!(next.map(|p| p.title), Some("Next".to_string()));
        assert_eq!(now_next(&pool, None, "x", 150_000), (None, None));
    }
}
