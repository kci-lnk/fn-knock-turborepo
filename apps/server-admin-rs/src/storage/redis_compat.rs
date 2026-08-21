#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Display,
    path::{Path, PathBuf},
    sync::Arc,
};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tokio_rusqlite::{
    Connection, OptionalExtension,
    rusqlite::{self, ToSql, params, params_from_iter},
};

use crate::storage::{StorageError, StorageResult, storage_error};

mod command;
mod connection;
mod connection_ops;
mod eval;
mod executor;
mod interface;
mod mobility_snapshot;
mod persistence;
mod primitives;
mod query;
mod schema;
mod shadow_sync;
mod transactions;

use eval::*;
use executor::*;
use interface::*;
use mobility_snapshot::*;
use persistence::*;
use primitives::*;
use query::*;
use schema::*;
use shadow_sync::*;
use transactions::*;

#[allow(unused_imports)]
pub(crate) use command::{Cmd, Pipeline, cmd, pipe};
#[allow(unused_imports)]
pub(crate) use interface::{
    AsyncCommands, CmdOutput, ConnectionManager, RedisError, RedisResult, streams,
};
pub(crate) use primitives::string_get_tx;
pub(crate) use transactions::{
    execute_command_in_transaction, hash_entries_in_transaction, hash_field_matches_in_transaction,
};

#[cfg(test)]
mod tests;
