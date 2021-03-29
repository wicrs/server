use auth::AuthError;
use permission::{ChannelPermission, HubPermission};
use reqwest::StatusCode;
use std::time::{SystemTime, UNIX_EPOCH};

use parse_display::{Display, FromStr};

use serde::{Deserialize, Serialize};

use uuid::Uuid;

/// Public API for performing user actions, should be used for creating API implementations like the HTTP API or similar.
pub mod api;
/// Authentication handling.
pub mod auth;
/// Message storage and retreival for channels.
pub mod channel;
/// Various objects for storing configuration.
pub mod config;
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

/// Errors related to data processing.
#[derive(Debug, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
#[serde(rename_all(
    serialize = "SCREAMING_SNAKE_CASE",
    deserialize = "SCREAMING_SNAKE_CASE"
))]
pub enum DataError {
    WriteFile,
    Deserialize,
    Directory,
    ReadFile,
    Serialize,
    DeleteFailed,
}

/// General errors that can occur when using the WICRS API.
#[derive(Debug, Serialize, Deserialize, actix::Message, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
#[rtype(result = "()")]
#[serde(rename_all(
    serialize = "SCREAMING_SNAKE_CASE",
    deserialize = "SCREAMING_SNAKE_CASE"
))]
pub enum ApiError {
    Muted,
    Banned,
    HubNotFound,
    ChannelNotFound,
    #[display("{}({0})")]
    MissingHubPermission(HubPermission),
    #[display("{}({0})")]
    MissingChannelPermission(ChannelPermission),
    NotInHub,
    UserNotFound,
    MemberNotFound,
    MessageNotFound,
    NotAuthenticated,
    GroupNotFound,
    InvalidName,
    UnexpectedServerArg,
    MessageTooBig,
    InvalidMessage,
    MessageSendFailed,
    CannotAuthenticate,
    Io,
    #[display("{}({0})")]
    Auth(AuthError),
    #[display("{}({0})")]
    Data(DataError),
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        Self::Auth(err)
    }
}

impl From<DataError> for ApiError {
    fn from(err: DataError) -> Self {
        Self::Data(err)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<&ApiError> for StatusCode {
    fn from(error: &ApiError) -> Self {
        match error {
            ApiError::NotAuthenticated => Self::UNAUTHORIZED,
            ApiError::InvalidName => Self::BAD_REQUEST,
            ApiError::Banned => Self::FORBIDDEN,
            ApiError::ChannelNotFound => Self::NOT_FOUND,
            ApiError::GroupNotFound => Self::NOT_FOUND,
            ApiError::HubNotFound => Self::NOT_FOUND,
            ApiError::MemberNotFound => Self::NOT_FOUND,
            ApiError::MessageNotFound => Self::NOT_FOUND,
            ApiError::Muted => Self::FORBIDDEN,
            ApiError::MissingChannelPermission(_) => Self::FORBIDDEN,
            ApiError::MissingHubPermission(_) => Self::FORBIDDEN,
            ApiError::NotInHub => Self::NOT_FOUND,
            ApiError::UserNotFound => Self::NOT_FOUND,
            ApiError::UnexpectedServerArg => Self::INTERNAL_SERVER_ERROR,
            ApiError::MessageTooBig => Self::BAD_REQUEST,
            ApiError::CannotAuthenticate => Self::INTERNAL_SERVER_ERROR,
            ApiError::InvalidMessage => Self::BAD_REQUEST,
            ApiError::MessageSendFailed => Self::INTERNAL_SERVER_ERROR,
            ApiError::Auth(error) => error.into(),
            ApiError::Data(_) => Self::INTERNAL_SERVER_ERROR,
            ApiError::Io => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

pub type Result<T, E = ApiError> = std::result::Result<T, E>;

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
        Err(ApiError::InvalidName)
    }
}

/// Checks that a hub member has a given permission and returns an error if it doesn't.
#[macro_export]
macro_rules! check_permission {
    ($member:expr, $perm:expr, $hub:expr) => {
        if !$member.has_permission($perm, &$hub) {
            return Err(ApiError::MissingHubPermission($perm));
        }
    };
    ($member:expr, $channel:expr, $perm:expr, $hub:expr) => {
        if !$member.has_channel_permission($channel, &$perm, &$hub) {
            return Err(ApiError::MissingChannelPermission($perm));
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
