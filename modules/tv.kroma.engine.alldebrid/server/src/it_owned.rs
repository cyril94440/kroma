use serde_json::json;

use crate::fake_alldebrid::{Fake, Resp};
use crate::it_engine::engine;
use crate::owned::Owned;

fn account(held: &'static [u64], status_code: u8, new_id: u64) -> Fake {
    Fake::start(move |req| match req.path.as_str() {
        "/v4.1/magnet/status" if req.form().contains_key("id") => Resp::data(json!({ "magnets": {
            "id": req.form()["id"], "filename": "x", "size": 1, "status": "s", "statusCode": status_code
        } })),
        "/v4.1/magnet/status" => {
            let magnets: Vec<_> = held
                .iter()
                .map(|id| json!({ "id": id, "filename": "mine", "size": 1, "status": "Ready", "statusCode": 4 }))
                .collect();
            Resp::data(json!({ "magnets": magnets }))
        }
        "/v4/magnet/upload" => Resp::data(json!({ "magnets": [{ "id": new_id, "ready": false }] })),
        "/v4/magnet/delete" => Resp::data(json!({ "message": "deleted" })),
        _ => Resp::failure("NOPE", "unexpected call"),
    })
}

#[test]
fn a_magnet_the_user_added_themselves_is_never_deleted_by_a_cancel() {
    let fake = account(&[5, 7], 1, 7);
    let dir = kroma_module_sdk::testing::temp_dir("ad-own-theirs");
    let ad = engine(&fake, dir.path());

    let id = ad.add("magnet:?xt=urn:btih:abc").unwrap();
    ad.remove(&id, true, false).unwrap();

    assert_eq!(id, "7");
    assert!(fake.calls_to("/v4/magnet/delete").is_empty());
}

#[test]
fn a_magnet_this_module_created_is_deleted_by_a_cancel_and_forgotten() {
    let fake = account(&[5], 1, 9);
    let dir = kroma_module_sdk::testing::temp_dir("ad-own-ours");
    let ad = engine(&fake, dir.path());

    let id = ad.add("magnet:?xt=urn:btih:abc").unwrap();
    assert!(Owned::new(dir.path()).has(&id));
    ad.remove(&id, true, false).unwrap();

    assert_eq!(fake.calls_to("/v4/magnet/delete").len(), 1);
    assert!(!Owned::new(dir.path()).has(&id));
    assert!(!dir.path().join("credentials/9.json").exists());
}

#[test]
fn a_finished_magnet_we_created_stays_unless_its_release_was_discarded() {
    let fake = account(&[], 4, 9);
    let dir = kroma_module_sdk::testing::temp_dir("ad-own-discard");
    let ad = engine(&fake, dir.path());
    let id = ad.add("magnet:?xt=urn:btih:abc").unwrap();

    ad.remove(&id, true, false).unwrap();
    assert!(fake.calls_to("/v4/magnet/delete").is_empty());
    ad.remove(&id, true, true).unwrap();

    assert_eq!(fake.calls_to("/v4/magnet/delete").len(), 1);
}

#[test]
fn when_the_account_cannot_be_listed_an_added_magnet_is_not_ours_to_delete() {
    let fake = Fake::start(|req| match req.path.as_str() {
        "/v4.1/magnet/status" => Resp::failure("INTERNAL_ERROR", "down"),
        "/v4/magnet/upload" => Resp::data(json!({ "magnets": [{ "id": 9, "ready": false }] })),
        _ => Resp::failure("NOPE", "unexpected call"),
    });
    let dir = kroma_module_sdk::testing::temp_dir("ad-own-nosnap");
    let ad = engine(&fake, dir.path());

    let id = ad.add("magnet:?xt=urn:btih:abc").unwrap();

    assert!(!Owned::new(dir.path()).has(&id));
}
