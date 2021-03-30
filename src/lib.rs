pub use error::{Result, Error};
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

/// List of characters that can be used in a username.
pub const NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// Maximum length of a name in characters.
pub const MAX_NAME_LENGTH: usize = 32;

/// Minimum length of a name in characters.
pub const MIN_NAME_LENGTH: usize = 1;

/// Maximum size of a message in bytes.
pub const MESSAGE_MAX_SIZE: usize = 4096;

/// Get the current time in milliseconds since Unix Epoch.
pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// Checks if a name is valid (not too long and only allowed characters).
pub fn is_valid_name(name: &str) -> bool {
    name.len() >= MIN_NAME_LENGTH
        && name.len() <= MAX_NAME_LENGTH
        && name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c))
}

/// Wraps `is_valid_name` to return a `Result<()>`.
///
/// # Errors
///
/// This function returns an error for any of the following reasons:
///
/// * The name is too long (maximum defined by [`MAX_NAME_LENGTH`]).
/// * The name is too short (minimum defined by [`MIN_NAME_LENGTH`]).
/// * The name contains characters not listed in [`NAME_ALLOWED_CHARS`].
pub fn check_name_validity(name: &str) -> Result<()> {
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
        if !$member.has_channel_permission($channel, &$perm, &$hub) {
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

#[cfg(test)]
mod tests {
    use super::is_valid_name;

    #[test]
    fn valid_username_check() {
        assert!(is_valid_name("a"));
        assert!(is_valid_name("Test_test tHAt-tester."));
        assert!(is_valid_name("1234567890"));
        assert!(is_valid_name("l33t 5p34k"));
        assert!(!is_valid_name(""));
        assert!(!is_valid_name("Test! @thing"));
        assert!(!is_valid_name("123456789111315171921232527293133"));
    }
}
