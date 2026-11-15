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
}
