use anyhow::Result;

use kroma_module_host::HostStorage;
use kroma_primitives::now_ms;

use crate::db;
use crate::model::RequestStatus;

use super::notify::publish;

/// Puts back to `wanted` the rows of `request_id` named in `wanted_ids` that a
/// download took (`grabbed`), and no others: a row another live download covers
/// stays as it is. The request's status is then read again from its rows, as the
/// availability pass reads it: all available, some available, otherwise
/// approved. A request that is denied, failed or still pending is left alone.
/// Announces nothing.
pub fn release_wanted<S: HostStorage>(
    state: &S,
    request_id: &str,
    wanted_ids: &[String],
) -> Result<Option<RequestStatus>> {
    let conn = state.db().get()?;
    let Some(req) = db::get_request(&conn, request_id)? else {
        return Ok(None);
    };
    let rows = db::wanted_for_request(&conn, request_id)?;
    drop(conn);
    let released: Vec<String> = rows
        .iter()
        .filter(|row| row.status == "grabbed" && wanted_ids.contains(&row.id))
        .map(|row| row.id.clone())
        .collect();
    db::set_wanted_status(state.db(), &released, "wanted", now_ms())?;
    if !matches!(
        req.status,
        RequestStatus::Approved
            | RequestStatus::Downloading
            | RequestStatus::PartiallyAvailable
            | RequestStatus::Available
    ) {
        return Ok(Some(req.status));
    }
    let available = rows
        .iter()
        .filter(|row| row.status == "available")
        .count();
    let status = if !rows.is_empty() && available == rows.len() {
        RequestStatus::Available
    } else if available > 0 {
        RequestStatus::PartiallyAvailable
    } else {
        RequestStatus::Approved
    };
    if status != req.status {
        db::set_request_status(state.db(), request_id, status, None, None, now_ms())?;
        publish(state, request_id, status);
    }
    Ok(Some(status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::test_fixtures::wanted;
    use crate::lifecycle::test_support::{insert_req, status_of_req, test_host};
    use crate::model::RequestKind;

    fn row_status(host: &crate::lifecycle::test_support::TestHost, id: &str) -> String {
        let conn = host.db().get().unwrap();
        db::wanted_for_request(&conn, "r1")
            .unwrap()
            .into_iter()
            .find(|w| w.id == id)
            .unwrap()
            .status
    }

    fn show_with(host: &crate::lifecycle::test_support::TestHost, rows: &[(&str, u32, &str)]) {
        insert_req(host, "r1", RequestKind::Show, 100, RequestStatus::Downloading);
        let wanted_rows: Vec<_> = rows
            .iter()
            .map(|(id, episode, status)| wanted(id, "r1", Some(1), Some(*episode), None, status))
            .collect();
        db::claim_wanted(host.db(), "r1", &wanted_rows, now_ms()).unwrap();
    }

    #[test]
    fn only_the_rows_the_download_took_go_back_and_another_downloads_row_stays() {
        let host = test_host();
        show_with(&host, &[("w1", 1, "grabbed"), ("w2", 2, "grabbed")]);

        release_wanted(&host, "r1", &["w1".to_string()]).unwrap();

        assert_eq!(row_status(&host, "w1"), "wanted");
        assert_eq!(row_status(&host, "w2"), "grabbed");
    }

    #[test]
    fn a_partly_available_request_is_not_forced_back_to_approved() {
        let host = test_host();
        show_with(&host, &[("w1", 1, "available"), ("w2", 2, "grabbed")]);

        let status = release_wanted(&host, "r1", &["w2".to_string()]).unwrap();

        assert_eq!(status, Some(RequestStatus::PartiallyAvailable));
        assert_eq!(status_of_req(&host, "r1"), RequestStatus::PartiallyAvailable);
    }

    #[test]
    fn with_nothing_available_the_request_is_approved_again() {
        let host = test_host();
        show_with(&host, &[("w1", 1, "grabbed")]);

        let status = release_wanted(&host, "r1", &["w1".to_string()]).unwrap();

        assert_eq!(status, Some(RequestStatus::Approved));
    }

    #[test]
    fn a_failed_request_is_left_failed() {
        let host = test_host();
        insert_req(&host, "r1", RequestKind::Movie, 100, RequestStatus::Failed);

        let status = release_wanted(&host, "r1", &[]).unwrap();

        assert_eq!(status, Some(RequestStatus::Failed));
    }

    #[test]
    fn an_unknown_request_is_nothing_to_release() {
        let host = test_host();

        assert_eq!(release_wanted(&host, "ghost", &[]).unwrap(), None);
    }
}
