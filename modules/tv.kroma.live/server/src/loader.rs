use std::collections::HashMap;

use anyhow::{Context, Result};
use serde_json::Value;

use crate::model::{Channel, ProviderId};
use crate::norm;
use crate::rules::Rules;

fn kept(rules: &Rules, name: &str, adult_flag: bool) -> bool {
    if norm::is_separator(name) {
        return false;
    }
    if !rules.family_filter {
        return true;
    }
    let normalised = norm::fold(name);
    !adult_flag && !rules.blocked_terms.iter().any(|term| normalised.contains(term.as_str()))
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

pub fn categories(body: &[u8]) -> HashMap<String, String> {
    let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(body) else {
        return HashMap::new();
    };
    items
        .iter()
        .map(|item| (text(item.get("category_id")), text(item.get("category_name"))))
        .filter(|(id, _)| !id.is_empty())
        .collect()
}

pub fn parse_xtream(provider: ProviderId, body: &[u8], groups: &HashMap<String, String>, rules: &Rules) -> Result<Vec<Channel>> {
    let items: Vec<Value> = serde_json::from_slice(body).context("the portal did not answer a channel list")?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let name = text(item.get("name"));
            let stream_id = integer(item.get("stream_id"))?;
            if name.is_empty() || !kept(rules, &name, text(item.get("is_adult")) == "1") {
                return None;
            }
            let group = groups.get(&text(item.get("category_id"))).map_or("", String::as_str);
            let epg = text(item.get("epg_channel_id"));
            Some(Channel::new(provider, stream_id, &name, Some(&epg), group))
        })
        .collect())
}

fn attribute(line: &str, name: &str) -> Option<String> {
    let marker = format!("{name}=\"");
    let start = line.find(&marker)? + marker.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

pub fn parse_m3u(provider: ProviderId, text: &str, rules: &Rules) -> Vec<Channel> {
    let mut out = Vec::new();
    let mut pending: Option<(String, Option<String>, String)> = None;
    for line in text.lines().map(str::trim) {
        if let Some(info) = line.strip_prefix("#EXTINF:") {
            let title = info.rsplit_once(',').map_or("", |(_, title)| title).trim();
            let name = attribute(info, "tvg-name").filter(|n| !n.is_empty()).unwrap_or_else(|| title.to_string());
            pending = Some((name, attribute(info, "tvg-id"), attribute(info, "group-title").unwrap_or_default()));
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Some((name, epg, group)) = pending.take() {
                if !name.is_empty() && kept(rules, &name, false) {
                    let id = out.len() as i64 + 1;
                    out.push(Channel::new(provider, id, &name, epg.as_deref(), &group).with_url(line));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules::default()
    }

    #[test]
    fn xtream_channels_keep_their_ids_guide_ids_and_groups() {
        let groups = categories(br#"[{"category_id":"3","category_name":"Sport"}]"#);
        let body = br#"[{"name":" TF1 ","stream_id":11,"epg_channel_id":"TF1.fr","category_id":"3","is_adult":"0"},
                        {"name":"Canal","stream_id":"12","epg_channel_id":"","category_id":"9"}]"#;

        let channels = parse_xtream(ProviderId::Primary, body, &groups, &rules()).unwrap();

        assert_eq!(channels.len(), 2);
        assert_eq!((channels[0].name.as_str(), channels[0].epg_id.as_deref(), channels[0].group.as_str()), ("TF1", Some("TF1.fr"), "Sport"));
        assert_eq!((channels[1].stream_id, channels[1].epg_id.clone()), (12, None));
        assert_eq!(channels[0].id(), "p:11");
    }

    #[test]
    fn the_family_filter_drops_the_flag_the_terms_and_the_separators() {
        let body = br######"[{"name":"Night XXX","stream_id":1},{"name":"Fine","stream_id":2,"is_adult":"1"},
                        {"name":"##### SPORT #####","stream_id":3},{"name":"Kids 18+","stream_id":4},{"name":"Fine","stream_id":5}]"######;

        let channels = parse_xtream(ProviderId::Primary, body, &HashMap::new(), &rules()).unwrap();

        assert_eq!(channels.iter().map(|c| c.stream_id).collect::<Vec<_>>(), [5]);
    }

    #[test]
    fn turning_the_family_filter_off_keeps_the_flagged_but_never_the_separators() {
        let mut open = rules();
        open.family_filter = false;
        let body = br######"[{"name":"Fine","stream_id":2,"is_adult":"1"},{"name":"### X ###","stream_id":3}]"######;

        let channels = parse_xtream(ProviderId::Primary, body, &HashMap::new(), &open).unwrap();

        assert_eq!(channels.len(), 1);
    }

    #[test]
    fn an_answer_that_is_not_a_list_is_an_error_without_the_body() {
        let error = parse_xtream(ProviderId::Primary, b"{\"user_info\":{\"auth\":0}}", &HashMap::new(), &rules()).unwrap_err();

        assert!(!format!("{error:#}").contains("auth"));
    }

    #[test]
    fn an_m3u_pairs_each_info_line_with_its_stream() {
        let text = "#EXTM3U\n#EXTINF:-1 tvg-id=\"tf1.fr\" tvg-name=\"FR: TF1 FHD\" group-title=\"News\",TF1\nhttp://h.test/a/b/1.ts\n#EXTINF:-1,France 2\nhttp://h.test/a/b/2.ts\n#EXTINF:-1,Orphan\n";

        let channels = parse_m3u(ProviderId::Backup, text, &rules());

        assert_eq!(channels.len(), 2);
        assert_eq!((channels[0].name.as_str(), channels[0].epg_id.as_deref(), channels[0].group.as_str()), ("FR: TF1 FHD", Some("tf1.fr"), "News"));
        assert_eq!(channels[1].url.as_deref(), Some("http://h.test/a/b/2.ts"));
        assert_eq!(channels[1].id(), "s:2");
    }
}
