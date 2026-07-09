use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthLoginMode {
    Totp,
    Password,
}

impl AuthLoginMode {
    pub(crate) fn from_storage(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("password") => Self::Password,
            _ => Self::Totp,
        }
    }

    pub(crate) fn from_api(value: &str) -> Option<Self> {
        match value.trim() {
            "totp" => Some(Self::Totp),
            "password" => Some(Self::Password),
            _ => None,
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Totp => "totp",
            Self::Password => "password",
        }
    }

    pub(crate) fn allows_totp_family(self) -> bool {
        self == Self::Totp
    }
}

impl fmt::Display for AuthLoginMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthMethod {
    Totp,
    Password,
    Passkey,
    Oidc,
}

impl AuthMethod {
    pub(crate) fn from_login_request(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("totp") {
            Some(Self::Totp)
        } else if value.eq_ignore_ascii_case("password") {
            Some(Self::Password)
        } else {
            None
        }
    }

    pub(crate) fn as_session_str(self) -> &'static str {
        match self {
            Self::Totp => "TOTP",
            Self::Password => "PASSWORD",
            Self::Passkey => "PASSKEY",
            Self::Oidc => "OIDC",
        }
    }

    pub(crate) fn matches_session_str(self, value: &str) -> bool {
        value.eq_ignore_ascii_case(self.as_session_str())
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_session_str())
    }
}
