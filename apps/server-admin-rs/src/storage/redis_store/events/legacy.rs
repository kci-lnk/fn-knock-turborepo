pub(in crate::storage::redis_store) const EVENTS_STREAM_KEY: &str = "fn_knock:events:stream";
pub(in crate::storage::redis_store) const EVENTS_INDEX_KEY: &str = "fn_knock:events:index";
pub(in crate::storage::redis_store) const EVENTS_DATA_PREFIX: &str = "fn_knock:events:data:";
pub(in crate::storage::redis_store) const EVENTS_DEDUPE_PREFIX: &str =
    crate::storage::typed_event_dedupe::DEDUPE_PREFIX;
pub(in crate::storage::redis_store) const EVENTS_STREAM_ID_PREFIX: &str =
    "fn_knock:events:stream-id:";
pub(in crate::storage::redis_store) const EVENT_LIST_SCAN_CHUNK_SIZE: isize = 200;
pub(in crate::storage::redis_store) const EVENT_CLEAR_CHUNK_SIZE: usize = 500;
pub(in crate::storage::redis_store) const MAX_EVENT_RETENTION_DAYS: i64 = 90;
