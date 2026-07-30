const AUTH_SESSION_PREFIX: &str = "fn_knock:session:";

pub(crate) fn session_key(session_id: &str) -> String {
    format!("{AUTH_SESSION_PREFIX}{session_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_auth_session_key() {
        assert_eq!(session_key("session-1"), "fn_knock:session:session-1");
    }
}
