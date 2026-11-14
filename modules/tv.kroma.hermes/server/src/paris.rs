const DAYS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const MONTHS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre",
    "novembre", "décembre",
];

pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub weekday: usize,
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn last_sunday_utc_ms(year: i64, month: u32) -> i64 {
    let last_day = if month == 12 { 31 } else { days_from_civil(year, month + 1, 1) - days_from_civil(year, month, 1) };
    let last = days_from_civil(year, month, last_day as u32);
    let weekday = (last + 3).rem_euclid(7);
    (last - (weekday + 1) % 7) * 86_400_000 + 3_600_000
}

fn offset_ms(utc_ms: i64) -> i64 {
    let year = civil_from_days(utc_ms.div_euclid(86_400_000)).0;
    let summer = utc_ms >= last_sunday_utc_ms(year, 3) && utc_ms < last_sunday_utc_ms(year, 10);
    if summer { 7_200_000 } else { 3_600_000 }
}

pub fn civil(utc_ms: i64) -> Civil {
    let local = utc_ms + offset_ms(utc_ms);
    let days = local.div_euclid(86_400_000);
    let minutes = local.rem_euclid(86_400_000) / 60_000;
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: (minutes / 60) as u32,
        minute: (minutes % 60) as u32,
        weekday: (days + 3).rem_euclid(7) as usize,
    }
}

pub fn clock_label(utc_ms: i64) -> String {
    let c = civil(utc_ms);
    format!("{:02}h{:02}", c.hour, c.minute)
}

pub fn date_label(utc_ms: i64) -> String {
    let c = civil(utc_ms);
    format!("{:02}/{:02}/{}", c.day, c.month, c.year)
}

pub fn dated_label(utc_ms: i64) -> String {
    let c = civil(utc_ms);
    format!("{:02}/{:02}/{} {:02}h{:02}", c.day, c.month, c.year, c.hour, c.minute)
}

pub fn long_label(utc_ms: i64) -> String {
    let c = civil(utc_ms);
    format!(
        "{} {} {} {}, {:02}h{:02}",
        DAYS[c.weekday],
        c.day,
        MONTHS[(c.month - 1) as usize],
        c.year,
        c.hour,
        c.minute
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winter_time_is_one_hour_ahead_of_utc() {
        assert_eq!(long_label(1_794_000_000_000), "vendredi 6 novembre 2026, 22h20");
    }

    #[test]
    fn summer_time_is_two_hours_ahead_of_utc() {
        assert_eq!(clock_label(1_782_000_000_000), "02h00");
        assert_eq!(long_label(1_782_000_000_000), "dimanche 21 juin 2026, 02h00");
    }

    #[test]
    fn the_changeover_follows_the_last_sunday_of_march_and_october() {
        assert_eq!(clock_label(1_774_746_000_000 - 1), "01h59");
        assert_eq!(clock_label(1_774_746_000_000), "03h00");
        assert_eq!(clock_label(1_792_890_000_000 - 1), "02h59");
        assert_eq!(clock_label(1_792_890_000_000), "02h00");
    }
}
