use super::Party;

pub const ROLE_TTL_MS: i64 = 600_000;

impl Party {
    pub(crate) fn note_role(&self, user_id: &str, admin: bool) {
        let now = self.now();
        let mut st = self.lock();
        if admin {
            st.admins.insert(user_id.to_string(), now);
        } else {
            st.admins.remove(user_id);
        }
    }

    pub(crate) fn is_admin_id(&self, user_id: &str) -> bool {
        let now = self.now();
        !user_id.is_empty() && self.lock().admins.get(user_id).is_some_and(|seen| now - seen < ROLE_TTL_MS)
    }

    pub(crate) fn require_admin_id(&self, user_id: &str) -> Result<(), &'static str> {
        if self.is_admin_id(user_id) {
            Ok(())
        } else {
            Err("adminOnly")
        }
    }

    pub(crate) fn require_log_access(&self, requested_by: &str, target: Option<&str>) -> Result<(), &'static str> {
        if self.is_admin_id(requested_by) || (!requested_by.is_empty() && target == Some(requested_by)) {
            Ok(())
        } else {
            Err("adminOnly")
        }
    }
}
