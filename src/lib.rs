use error::{Error, Result};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Public API for performing user actions, should be used for creating API implementations like the HTTP API or similar.
pub mod api;
/// Authentication handling.
pub mod auth;
/// Message storage and retreival for channels.
pub mod channel;
/// Various objects for storing configuration.
pub mod config;
/// Errors
pub mod error;
/// Definition of the HTTP API.
pub mod httpapi;
/// Hubs, permission management, channel management and member management.
pub mod hub;
/// Permissions are defined here.
pub mod permission;
/// Server implementation.
pub mod server;
/// User management.
pub mod user;
/// Definition of the WebSocket API.
pub mod websocket;

/// String to identify the version of the library, used for external requests.
pub const USER_AGENT_STRING: &str = concat!("WICRS Server ", env!("CARGO_PKG_VERSION"));

/// Maximum size of a username in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_NAME_SIZE: usize = 128;

/// Maximum size of a user status in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_STATUS_SIZE: usize = 128;

/// Maximum size of a description in bytes. Clients should be able to accept larger and smaller values.
pub const MAX_DESCRIPTION_SIZE: usize = 8192;

/// Maximum size of a message in bytes. Clients should be able to accept larger and smaller values.
pub const MESSAGE_MAX_SIZE: usize = 8192;

/// Get the current time in milliseconds since Unix Epoch.
pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// Checks if a name is valid (not too long and only allowed characters).
pub fn is_valid_name(name: &str) -> bool {
    name.as_bytes().len() <= MAX_NAME_SIZE
}

/// Wraps `is_valid_name` to return a `Result<()>`.
///
/// # Errors
///
/// This function returns an error for any of the following reasons:
///
/// * The name is too big (maximum in bytes defined by [`MAX_NAME_SIZE`]).
pub fn check_name_validity(name: &str) -> Result {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(Error::InvalidName)
    }
}

/// Checks that a hub member has a given permission and returns an error if it doesn't.
#[macro_export]
macro_rules! check_permission {
    ($member:expr, $perm:expr, $hub:expr) => {
        if !$member.has_permission($perm, &$hub) {
            return Err(Error::MissingHubPermission($perm));
        }
    };
    ($member:expr, $channel:expr, $perm:expr, $hub:expr) => {
        if !$member.has_channel_permission($channel, $perm, &$hub) {
            return Err(Error::MissingChannelPermission($perm));
        }
    };
}

/// Type used to represent IDs throughout wicrs.
pub type ID = Uuid;

/// Generates a new random ID.
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}
