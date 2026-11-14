use super::*;

fn body(season: Option<u32>, episode: Option<u32>) -> Body {
    Body { season, episode }
}

#[test]
fn a_movie_needs_no_episode() {
    let scope = scope_of("movie", 603, body(Some(3), Some(4))).unwrap();

    assert_eq!(
        scope,
        Scope {
            kind: RequestKind::Movie,
            tmdb_id: 603,
            season: None,
            episode: None
        }
    );
}

#[test]
fn a_show_with_no_episode_named_starts_at_the_first() {
    let scope = scope_of("tv", 1396, body(None, None)).unwrap();

    assert_eq!((scope.season, scope.episode), (Some(1), Some(1)));
    assert_eq!(scope_of("show", 1396, body(None, None)).unwrap(), scope);
}

#[test]
fn a_show_names_both_numbers_or_neither() {
    assert!(scope_of("tv", 1, body(Some(2), None)).is_err());
    assert!(scope_of("tv", 1, body(None, Some(2))).is_err());
    assert!(scope_of("tv", 1, body(Some(0), Some(1))).is_err());
    assert_eq!(
        scope_of("tv", 1, body(Some(2), Some(5))).unwrap().episode,
        Some(5)
    );
}

#[test]
fn an_unknown_kind_or_a_zero_id_is_refused() {
    assert!(scope_of("album", 1, body(None, None)).is_err());
    assert!(scope_of("movie", 0, body(None, None)).is_err());
}

#[test]
fn every_outcome_answers_in_the_shape_the_clients_read() {
    assert_eq!(
        answer(
            Outcome::Ready {
                item_id: "i".into(),
                file_id: Some("f".into())
            },
            "en"
        ),
        json!({ "status": "ready", "itemId": "i", "fileId": "f" })
    );
    assert_eq!(
        answer(
            Outcome::Requested {
                request_id: Some("r".into())
            },
            "en"
        ),
        json!({ "status": "requested", "requestId": "r" })
    );
    let unavailable = answer(
        Outcome::Unavailable {
            reason: Reason::NoRelease,
            request_id: None,
        },
        "fr",
    );
    assert_eq!(unavailable["status"], "unavailable");
    assert_eq!(unavailable["reason"], "noRelease");
    assert!(unavailable["message"]
        .as_str()
        .unwrap()
        .contains("Rien ne correspond"));
}
