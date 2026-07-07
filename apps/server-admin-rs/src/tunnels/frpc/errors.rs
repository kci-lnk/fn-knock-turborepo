use super::*;

pub(super) fn frpc_error(status: StatusCode, message: impl Into<String>) -> FrpcHttpError {
    FrpcHttpError {
        status,
        message: message.into(),
    }
}

pub(super) fn frpc_internal(error: impl std::fmt::Display) -> FrpcHttpError {
    frpc_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

pub(super) fn frpc_validation(message: impl Into<String>) -> FrpcHttpError {
    frpc_error(StatusCode::BAD_REQUEST, message)
}

pub(super) fn frpc_not_found(id: &str) -> FrpcHttpError {
    frpc_error(
        StatusCode::NOT_FOUND,
        format!("FRPC instance not found: {id}"),
    )
}

impl From<anyhow::Error> for FrpcHttpError {
    fn from(value: anyhow::Error) -> Self {
        frpc_internal(value)
    }
}

impl From<std::io::Error> for FrpcHttpError {
    fn from(value: std::io::Error) -> Self {
        frpc_internal(value)
    }
}

impl From<redis::RedisError> for FrpcHttpError {
    fn from(value: redis::RedisError) -> Self {
        frpc_internal(value)
    }
}

impl From<serde_json::Error> for FrpcHttpError {
    fn from(value: serde_json::Error) -> Self {
        frpc_internal(value)
    }
}
