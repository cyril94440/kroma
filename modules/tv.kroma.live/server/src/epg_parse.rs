use std::io::Read;

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

use crate::db::StoredProgramme;
use crate::norm;

pub const PAST_SECONDS: i64 = 2 * 3600;
pub const FUTURE_SECONDS: i64 = 14 * 3600;
const DESCRIPTION_CHARS: usize = 300;

pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

pub fn parse_timestamp(text: &str) -> Option<i64> {
    let text = text.trim();
    let digits = text.get(..14)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number = |from: usize, to: usize| digits[from..to].parse::<i64>().ok();
    let (year, month, day, hour, minute, second) = (number(0, 4)?, number(4, 6)?, number(6, 8)?, number(8, 10)?, number(10, 12)?, number(12, 14)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let offset = text[14..].trim();
    let offset_seconds = if offset.len() == 5 && (offset.starts_with('+') || offset.starts_with('-')) {
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        let hours: i64 = offset[1..3].parse().ok()?;
        let minutes: i64 = offset[3..5].parse().ok()?;
        sign * (hours * 3600 + minutes * 60)
    } else {
        0
    };
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds)
}

pub fn read_body(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::with_capacity(bytes.len() * 8);
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut out).context("the guide is not valid gzip")?;
        return Ok(out);
    }
    Ok(bytes.to_vec())
}

#[derive(Default)]
struct Building {
    epg_id: String,
    start: i64,
    stop: i64,
    title: String,
    subtitle: String,
    description: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Subtitle,
    Description,
}

pub fn parse_xmltv(xml: &[u8], now_s: i64) -> Result<Vec<StoredProgramme>> {
    let (low, high) = (now_s - PAST_SECONDS, now_s + FUTURE_SECONDS);
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().check_end_names = false;
    reader.config_mut().trim_text(false);
    let mut out = Vec::new();
    let mut current: Option<Building> = None;
    let mut field: Option<Field> = None;
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf).context("the guide is not valid XML")? {
            Event::Start(tag) => {
                text.clear();
                match tag.local_name().as_ref() {
                    "programme" => current = begin(&tag),
                    "title" if current.is_some() => field = Some(Field::Title),
                    "sub-title" if current.is_some() => field = Some(Field::Subtitle),
                    "desc" if current.is_some() => field = Some(Field::Description),
                    _ => {}
                }
            }
            Event::Text(chunk) if field.is_some() => text.push_str(&chunk),
            Event::CData(chunk) if field.is_some() => text.push_str(&chunk),
            Event::GeneralRef(entity) if field.is_some() => {
                text.push_str(&quick_xml::escape::unescape(&format!("&{};", &*entity)).map(|c| c.into_owned()).unwrap_or_default());
            }
            Event::End(tag) => match tag.local_name().as_ref() {
                "programme" => {
                    if let Some(building) = current.take() {
                        out.extend(finish(building, low, high));
                    }
                    field = None;
                }
                "title" | "sub-title" | "desc" => {
                    if let (Some(building), Some(which)) = (current.as_mut(), field.take()) {
                        let target = match which {
                            Field::Title => &mut building.title,
                            Field::Subtitle => &mut building.subtitle,
                            Field::Description => &mut building.description,
                        };
                        if target.is_empty() {
                            *target = text.trim().to_string();
                        }
                    }
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    out.sort_by(|a, b| (a.epg_id.as_str(), a.start).cmp(&(b.epg_id.as_str(), b.start)));
    Ok(out)
}

fn begin(tag: &quick_xml::events::BytesStart<'_>) -> Option<Building> {
    let mut building = Building::default();
    for attribute in tag.attributes().flatten() {
        let value = attribute.normalized_value(XmlVersion::Implicit1_0).ok()?.into_owned();
        match attribute.key.local_name().as_ref() {
            "start" => building.start = parse_timestamp(&value)?,
            "stop" => building.stop = parse_timestamp(&value)?,
            "channel" => building.epg_id = value.to_lowercase(),
            _ => {}
        }
    }
    Some(building)
}

fn finish(building: Building, low: i64, high: i64) -> Option<StoredProgramme> {
    if building.epg_id.is_empty() || building.title.is_empty() || building.stop < low || building.start > high {
        return None;
    }
    let mut description: String = building.description.chars().take(DESCRIPTION_CHARS).collect();
    if !building.subtitle.is_empty() && !description.to_lowercase().contains(&building.subtitle.to_lowercase()) {
        description = if description.is_empty() { building.subtitle.clone() } else { format!("{} — {description}", building.subtitle) };
    }
    let hay = norm::fold(&[building.title.as_str(), building.subtitle.as_str(), description.as_str()].iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join(" "));
    Some(StoredProgramme { epg_id: building.epg_id, start: building.start, stop: building.stop, title: building.title, hay, description })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmltv_times_carry_their_offset() {
        assert_eq!(parse_timestamp("20260707090500 +0200"), Some(1_783_407_900));
        assert_eq!(parse_timestamp("20260707070500 +0000"), Some(1_783_407_900));
        assert_eq!(parse_timestamp("20260707070500"), Some(1_783_407_900));
        assert_eq!(parse_timestamp("20260707000500 -0700"), Some(1_783_407_900));
        assert_eq!(parse_timestamp("nope"), None);
        assert_eq!(parse_timestamp("20261307070500 +0000"), None);
    }

    const NOW: i64 = 1_783_407_900;

    fn xml(programmes: &str) -> Vec<u8> {
        format!("<?xml version=\"1.0\"?><tv><channel id=\"TF1.fr\"/>{programmes}</tv>").into_bytes()
    }

    #[test]
    fn only_the_window_is_kept_and_ids_are_lowercased() {
        let body = xml(
            "<programme start=\"20260707020000 +0000\" stop=\"20260707030000 +0000\" channel=\"TF1.fr\"><title>Too old</title></programme>\
             <programme start=\"20260707070000 +0000\" stop=\"20260707080000 +0000\" channel=\"TF1.fr\"><title>Journal</title><sub-title>Edition</sub-title><desc>Les infos &amp; la météo</desc></programme>\
             <programme start=\"20260708070000 +0000\" stop=\"20260708080000 +0000\" channel=\"TF1.fr\"><title>Tomorrow</title></programme>\
             <programme start=\"20260707090000 +0000\" stop=\"20260707100000 +0000\" channel=\"\"><title>No channel</title></programme>\
             <programme start=\"20260707090000 +0000\" stop=\"20260707100000 +0000\" channel=\"M6.fr\"><title></title></programme>",
        );

        let rows = parse_xmltv(&body, NOW).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].epg_id.as_str(), rows[0].title.as_str()), ("tf1.fr", "Journal"));
        assert_eq!(rows[0].description, "Edition — Les infos & la météo");
        assert!(rows[0].hay.contains("meteo") && rows[0].hay.contains("edition"));
    }

    #[test]
    fn a_gzipped_guide_is_read_like_a_plain_one() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&xml("<programme start=\"20260707070000 +0000\" stop=\"20260707080000 +0000\" channel=\"a\"><title>X</title></programme>")).unwrap();
        let zipped = encoder.finish().unwrap();

        let rows = parse_xmltv(&read_body(&zipped).unwrap(), NOW).unwrap();

        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn a_body_that_is_not_xml_is_an_error_and_a_broken_programme_is_skipped() {
        assert!(read_body(&[0x1f, 0x8b, 0, 0]).is_err());
        let body = xml("<programme start=\"garbage\" stop=\"20260707080000 +0000\" channel=\"a\"><title>X</title></programme><programme start=\"20260707070000 +0000\" stop=\"20260707080000 +0000\" channel=\"a\"><title>Y</title></programme>");

        let rows = parse_xmltv(&body, NOW).unwrap();

        assert_eq!(rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["Y"]);
    }
}
