use anyhow::Result;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::requests::lifecycle::fail_request;

const FAILED_GRABS_BEFORE_GIVING_UP: i64 = 3;

pub(super) fn fail_exhausted_requests<S: HostStorage>(
    state: &S,
    log: &dyn Fn(String),
) -> Result<usize> {
    let conn = state.db().get()?;
    let mut stmt = conn.prepare(
        "SELECT request_id FROM downloads WHERE request_id IS NOT NULL \
         GROUP BY request_id \
         HAVING SUM(status IN ('failed', 'error')) >= ?1 \
            AND SUM(status NOT IN ('failed', 'error', 'removed')) = 0",
    )?;
    let exhausted: Vec<String> = stmt
        .query_map([FAILED_GRABS_BEFORE_GIVING_UP], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);
    drop(conn);
    let mut failed = 0;
    for id in exhausted {
        match fail_request(state, &id) {
            Ok(true) => {
                log(format!(
                    "request {id} lost {FAILED_GRABS_BEFORE_GIVING_UP} downloads; marked failed"
                ));
                failed += 1;
            }
            Ok(false) => {}
            Err(e) => log(format!("could not mark request {id} failed: {e:#}")),
        }
    }
    Ok(failed)
}
