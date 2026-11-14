use crate::fake_alldebrid::{Fake, Resp};
use crate::it_engine::engine;
use crate::it_instant::{account_holding, candidates, deleted, upload_answer, WIDE};

#[test]
fn an_id_older_than_the_newest_the_account_listed_is_never_deleted() {
    let fake = account_holding(&[], &[], &[5000]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-older");

    engine(&fake, dir.path())
        .instant(&candidates(3), WIDE)
        .unwrap();

    assert!(deleted(&fake).is_empty());
}

#[test]
fn a_magnet_the_account_already_held_is_never_deleted_when_it_is_not_chosen() {
    let fake = account_holding(&[0], &[], &[1001, 1002]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-held-unchosen");

    let found = engine(&fake, dir.path())
        .instant(&candidates(4), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.key, "rel-0");
    assert!(deleted(&fake).is_empty());
}

#[test]
fn a_magnet_the_account_already_held_can_be_chosen_and_stays() {
    let fake = account_holding(&[1], &[], &[1001]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-held-chosen");

    let found = engine(&fake, dir.path())
        .instant(&candidates(3), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.client_ref, "1001");
    assert_eq!(deleted(&fake), ["1002"]);
}

#[test]
fn nothing_ready_still_spares_what_the_account_held() {
    let fake = account_holding(&[], &[], &[1000, 1001, 1002]);
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-held-none");

    let found = engine(&fake, dir.path()).instant(&candidates(3), WIDE).unwrap();

    assert_eq!(found, None);
    assert!(deleted(&fake).is_empty());
}

#[test]
fn when_the_account_cannot_be_listed_nothing_is_deleted_but_a_ready_one_is_still_found() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::failure("INTERNAL_ERROR", "down"),
        "/v4/magnet/upload" => upload_answer(req, &[1], &[]),
        _ => Resp::failure("NOPE", "unexpected call"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-instant-no-snapshot");

    let found = engine(&fake, dir.path())
        .instant(&candidates(3), WIDE)
        .unwrap()
        .unwrap();

    assert_eq!(found.client_ref, "1001");
    assert!(fake.calls_to("/v4/magnet/delete").is_empty());
}

#[test]
fn the_account_listing_reads_ids_given_as_numbers_or_strings() {
    let fake = Fake::start(|_| {
        Resp::data(
            serde_json::json!({ "magnets": [{ "id": 5 }, { "id": "6" }, { "filename": "x" }] }),
        )
    });
    let api = crate::api::Api::new(&fake.base, "k").unwrap();

    let ids = api.magnet_ids().unwrap();

    assert_eq!(ids.len(), 2);
    assert!(ids.contains("5") && ids.contains("6"));
    assert!(fake.calls_to("/v4.1/magnet/status")[0].query.is_empty());
}
