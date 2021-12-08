#[cfg(feature = "server")]
#[macro_use]
extern crate log;

#[cfg(feature = "server")]
use std::sync::Arc;

#[cfg(feature = "server")]
use error::{Error, Result};
#[cfg(feature = "server")]
use server::Server;
use uuid::Uuid;
#[cfg(feature = "server")]
use xactor::Actor;

/// Message storage and retreival for channels.
pub mod channel;
/// Various objects for storing configuration.
#[cfg(feature = "server")]
pub mod config;
/// Errors
pub mod error;
/// GraphQL model definition.
#[cfg(feature = "graphql")]
pub mod graphql_model;
/// Definition of the HTTP API.
#[cfg(feature = "server")]
pub mod httpapi;
/// Hubs, permission management, channel management and member management.
pub mod hub;
/// Permissions are defined here.
pub mod permission;
/// Public API exports.
pub mod prelude;
/// Server implementation.
#[cfg(feature = "server")]
pub mod server;
/// Definition of the WebSocket API.
#[cfg(feature = "server")]
pub mod websocket;

#[cfg(feature = "server")]
use prelude::{check_name_validity, new_id};

/// Maximum size of a username in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_NAME_SIZE: usize = 128;

/// Maximum size of a user status in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_STATUS_SIZE: usize = 128;

/// Maximum size of a description in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_DESCRIPTION_SIZE: usize = 8192;

/// Maximum size of a message in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_MESSAGE_SIZE: usize = 8192;

/// How long to wait before commiting new messages to the tantivy search engine in milliseconds, this takes a lot of time, which is why it should be done only periodically.
pub const TANTIVY_COMMIT_THRESHOLD: u8 = 10;

/// Starts WICRS Server in the current directory loading the configuration from `config.json`.
#[cfg(feature = "server")]
pub async fn start() -> Result {
    let config = config::load_config("config.json");
    if std::fs::create_dir_all("data").is_err() {
        Err(Error::from("Failed to create data directory.".to_string()))
    } else {
        let server = Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?;
        httpapi::start(config, Arc::new(server)).await
    }
}

/// Checks that a hub member has a given permission and returns an error if it doesn't.
#[cfg(feature = "server")]
#[macro_export]
macro_rules! check_permission {
    ($member:expr, $perm:expr, $hub:expr) => {
        if !$member.has_permission($perm, &$hub) {
            return Err(crate::error::ApiError::MissingHubPermission { permission: $perm }.into());
        }
    };
    ($member:expr, $channel:expr, $perm:expr, $hub:expr) => {
        if !$member.has_channel_permission($channel, $perm, &$hub) {
            return Err(
                crate::error::ApiError::MissingChannelPermission { permission: $perm }.into(),
            );
        }
    };
}

/// Type used to represent IDs of non user objects throughout wicrs.
#[allow(clippy::upper_case_acronyms)]
pub type ID = Uuid;

#[cfg(test)]
pub mod test {
    use super::*;
    pub use channel::test::*;
    use chrono::{DateTime, TimeZone, Utc};
    pub use hub::test::*;
    use uuid::Uuid;

    lazy_static::lazy_static! {
        pub static ref TEST_USER_ID: Uuid = Uuid::from_u128(2345678901);
        pub static ref TEST_GROUP_ID: Uuid = Uuid::from_u128(3456789012);
        pub static ref TEST_CHANNEL_ID: Uuid = Uuid::from_u128(4567890123);
        pub static ref TEST_MESSAGE_ID: Uuid = Uuid::from_u128(5678901234);
    }

    pub fn utc_unix_zero() -> DateTime<Utc> {
        chrono::Utc.timestamp(0, 0)
    }
}
