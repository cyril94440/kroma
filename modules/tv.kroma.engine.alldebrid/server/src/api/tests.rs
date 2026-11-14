use super::*;

#[test]
fn a_blank_key_is_refused_and_a_blank_base_falls_back_to_the_real_api() {
    assert!(Api::new("", "  ").is_err());

    let api = Api::new("  ", " key ").unwrap();

    assert_eq!(api.url("/v4/user"), "https://api.alldebrid.com/v4/user");
}

#[test]
fn a_base_override_loses_its_trailing_slash() {
    let api = Api::new("http://127.0.0.1:9/", "k").unwrap();

    assert_eq!(api.url("/v4/user"), "http://127.0.0.1:9/v4/user");
}

#[test]
fn ids_arrive_as_numbers_or_strings() {
    assert_eq!(wire::id_text(&serde_json::json!(12)), Some("12".into()));
    assert_eq!(wire::id_text(&serde_json::json!("12")), Some("12".into()));
    assert_eq!(wire::id_text(&serde_json::json!(null)), None);
    assert_eq!(wire::id_text(&serde_json::json!("")), None);
}

#[test]
fn only_the_unknown_magnet_code_reads_as_unknown() {
    let refused = |code: &str| {
        anyhow::Error::new(Refused {
            code: code.into(),
            message: String::new(),
        })
    };

    assert!(is_unknown_magnet(&refused("MAGNET_INVALID_ID")));
    assert!(!is_unknown_magnet(&refused("AUTH_BAD_APIKEY")));
    assert!(!is_unknown_magnet(&anyhow!("other")));
}
