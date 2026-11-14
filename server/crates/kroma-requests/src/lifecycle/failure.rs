use anyhow::Result;

use kroma_module_host::HostStorage;
use kroma_primitives::now_ms;

use crate::db;
use crate::model::RequestStatus;

use super::notify::{notify_transition, publish};

/// Marks an approved request whose acquisition gave up as failed and tells the
/// requester and the people who can act on it. Returns whether the request
/// changed: a request already failed, denied, pending or on disk is left alone,
/// so a repeated call announces nothing twice.
pub fn fail_request<S: HostStorage>(state: &S, id: &str) -> Result<bool> {
    let conn = state.db().get()?;
    let Some(req) = db::get_request(&conn, id)? else {
        return Ok(false);
    };
    drop(conn);
    if req.status != RequestStatus::Approved {
        return Ok(false);
    }
    db::set_request_status(state.db(), id, RequestStatus::Failed, None, None, now_ms())?;
    publish(state, id, RequestStatus::Failed);
    notify_transition(state, id, RequestStatus::Failed);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::test_support::{insert_req_by, status_of_req, test_host};
    use crate::model::{NotificationEvent, RequestKind};

    #[test]
    fn an_approved_request_fails_and_both_audiences_are_told() {
        let host = test_host();
        insert_req_by(&host, "r1", RequestKind::Movie, 1, RequestStatus::Approved, Some("u1"));

        let changed = fail_request(&host, "r1").unwrap();

        assert!(changed);
        assert_eq!(status_of_req(&host, "r1"), RequestStatus::Failed);
        let events: Vec<_> = host.notifications().iter().map(|(_, s)| s.event).collect();
        assert_eq!(
            events,
            [NotificationEvent::RequestFailed, NotificationEvent::RequestFailed]
        );
    }

    #[test]
    fn failing_a_request_twice_announces_it_once() {
        let host = test_host();
        insert_req_by(&host, "r1", RequestKind::Movie, 1, RequestStatus::Approved, Some("u1"));

        fail_request(&host, "r1").unwrap();
        let again = fail_request(&host, "r1").unwrap();

        assert!(!again);
        assert_eq!(host.notifications().len(), 2);
    }

    #[test]
    fn a_request_that_is_not_approved_is_left_alone() {
        let host = test_host();
        insert_req_by(&host, "r1", RequestKind::Movie, 1, RequestStatus::Available, Some("u1"));
        insert_req_by(&host, "r2", RequestKind::Movie, 2, RequestStatus::Denied, Some("u1"));

        let changed = [fail_request(&host, "r1").unwrap(), fail_request(&host, "r2").unwrap()];

        assert_eq!(changed, [false, false]);
        assert_eq!(status_of_req(&host, "r1"), RequestStatus::Available);
        assert!(host.notifications().is_empty());
    }

    #[test]
    fn an_unknown_request_is_not_an_error() {
        let host = test_host();

        assert!(!fail_request(&host, "ghost").unwrap());
    }
}
