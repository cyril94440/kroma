fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

pub fn parse_secs(text: &str) -> Option<f64> {
    let text = text.trim();
    let (date, rest) = text.split_once('T')?;
    let mut parts = date.split('-');
    let (year, month, day): (i64, i64, i64) = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    let split = rest.find(['Z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(split);
    let mut fields = clock.split(':');
    let hour: i64 = fields.next()?.parse().ok()?;
    let minute: i64 = fields.next()?.parse().ok()?;
    let seconds: f64 = fields.next().map_or(Some(0.0), |s| s.parse().ok())?;
    let offset = match zone.as_bytes().first() {
        Some(b'+') | Some(b'-') => {
            let digits: String = zone[1..].chars().filter(char::is_ascii_digit).collect();
            let (hours, minutes): (i64, i64) = (
                digits.get(..2)?.parse().ok()?,
                digits.get(2..4).unwrap_or("00").parse().ok()?,
            );
            (hours * 3600 + minutes * 60) * if zone.starts_with('-') { -1 } else { 1 }
        }
        _ => 0,
    };
    Some(
        (days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 - offset) as f64
            + seconds,
    )
}

pub fn day_key(secs: f64) -> String {
    let days = (secs / 86_400.0).floor() as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}{month:02}{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_espn_minutes_and_seconds_and_offsets() {
        assert_eq!(parse_secs("2026-11-03T20:00Z"), Some(1_793_736_000.0));
        assert_eq!(parse_secs("2026-11-03T20:00:30Z"), Some(1_793_736_030.0));
        assert_eq!(
            parse_secs("2026-11-03T21:00:00+01:00"),
            Some(1_793_736_000.0)
        );
        assert_eq!(parse_secs("later"), None);
    }

    #[test]
    fn day_key_names_the_utc_day() {
        assert_eq!(day_key(1_793_736_000.0), "20261103");
        assert_eq!(day_key(1_793_750_399.0), "20261103");
        assert_eq!(day_key(1_793_750_400.0), "20261104");
    }
}
