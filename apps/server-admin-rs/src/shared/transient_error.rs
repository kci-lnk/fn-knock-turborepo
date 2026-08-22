pub(crate) fn is_transient_runtime_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "timeout expired",
        "timed out",
        "deadline exceeded",
        "deadlineexceeded",
        "returned 500 internal server error",
        "returned 502 bad gateway",
        "returned 503 service unavailable",
        "returned 504 gateway timeout",
        "status: unavailable",
        "transport error",
        "connection refused",
        "connection reset",
        "database is locked",
        "database is busy",
        "disk i/o error",
        "temporarily unavailable",
    ]
    .iter()
    .any(|marker| error.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_error_classification_is_conservative() {
        for error in [
            "Timeout expired",
            "request timed out",
            "deadline exceeded",
            "returned 500 Internal Server Error",
            "returned 502 Bad Gateway",
            "returned 503 Service Unavailable",
            "returned 504 Gateway Timeout",
            "status: Unavailable",
            "transport error",
            "connection refused",
            "connection reset by peer",
            "database is locked",
            "database is busy",
            "disk I/O error",
            "temporarily unavailable",
        ] {
            assert!(
                is_transient_runtime_error(error),
                "expected transient: {error}"
            );
        }
        for error in [
            "returned 400 Bad Request",
            "returned 401 Unauthorized",
            "invalid host rule",
        ] {
            assert!(
                !is_transient_runtime_error(error),
                "expected permanent: {error}"
            );
        }
    }
}
