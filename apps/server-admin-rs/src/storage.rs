pub(crate) mod legacy_redis_migration;
pub(crate) mod redis_compat;
pub(crate) mod redis_store;

pub(crate) use redis_store as store;

pub(crate) type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, thiserror::Error)]
pub(crate) enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] tokio_rusqlite::rusqlite::Error),
    #[error("sqlite worker is closed")]
    SqliteConnectionClosed,
    #[error("sqlite close error: {0}")]
    SqliteClose(tokio_rusqlite::rusqlite::Error),
    #[error("storage serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("storage io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

impl From<tokio_rusqlite::Error<StorageError>> for StorageError {
    fn from(value: tokio_rusqlite::Error<StorageError>) -> Self {
        match value {
            tokio_rusqlite::Error::ConnectionClosed => Self::SqliteConnectionClosed,
            tokio_rusqlite::Error::Close((_, error)) => Self::SqliteClose(error),
            tokio_rusqlite::Error::Error(error) => error,
            _ => Self::Message("unknown sqlite worker error".to_string()),
        }
    }
}

impl From<tokio_rusqlite::Error> for StorageError {
    fn from(value: tokio_rusqlite::Error) -> Self {
        match value {
            tokio_rusqlite::Error::ConnectionClosed => Self::SqliteConnectionClosed,
            tokio_rusqlite::Error::Close((_, error)) => Self::SqliteClose(error),
            tokio_rusqlite::Error::Error(error) => Self::Sqlite(error),
            _ => Self::Message("unknown sqlite worker error".to_string()),
        }
    }
}

pub(crate) fn storage_error(message: impl Into<String>) -> StorageError {
    StorageError::Message(message.into())
}
