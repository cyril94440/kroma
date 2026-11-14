use anyhow::Result;
use kroma_module_sdk::host::HostStorage;
use kroma_module_sdk::requests::lifecycle::fail_request;

const FAILED_GRABS_BEFORE_GIVING_UP: i64 = 3;
const SINCE_KEY: &str = "give_up_since";

/// The moment the rule first ran: failures before it were never counted, so an
/// upgrade does not fail every request that already had three of them.
fn counted_since<S: HostStorage>(state: &S, now: i64) -> Result<i64> {
    let conn = state.store().get()?;
    conn.execute(
        "INSERT OR IGNORE INTO rule_settings (key, value) VALUES (?1, ?2)",
        rusqlite::params![SINCE_KEY, now.to_string()],
    )?;
    let value: String = conn.query_row(
        "SELECT value FROM rule_settings WHERE key = ?1",
        [SINCE_KEY],
        |r| r.get(0),
    )?;
    Ok(value.parse().unwrap_or(now))
}

pub(super) fn fail_exhausted_requests<S: HostStorage>(
    state: &S,
    log: &dyn Fn(String),
) -> Result<usize> {
    let since = counted_since(state, kroma_module_sdk::primitives::now_ms())?;
    let conn = state.db().get()?;
    let mut stmt = conn.prepare(
        "SELECT d.request_id FROM downloads d JOIN requests r ON r.id = d.request_id \
         GROUP BY d.request_id \
         HAVING SUM(d.status IN ('failed', 'error') AND d.grabbed_at >= ?2 AND d.grabbed_at >= r.updated_at) >= ?1 \
            AND SUM(d.status NOT IN ('failed', 'error', 'removed')) = 0",
    )?;
    let exhausted: Vec<String> = stmt
        .query_map(rusqlite::params![FAILED_GRABS_BEFORE_GIVING_UP, since], |r| r.get(0))?
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
