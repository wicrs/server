#![feature(proc_macro_hygiene, decl_macro)]
#![feature(in_band_lifetimes)]

use auth::Auth;
use futures::lock::Mutex;
use prelude::{ChannelPermission, HubPermission};
use reqwest::StatusCode;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
#[macro_use]
extern crate serial_test;

pub mod api;
pub mod auth;
pub mod channel;
pub mod config;
pub mod httpapi;
pub mod hub;
pub mod permission;
pub mod user;

use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum DataError {
    #[error("server was unable to write new data to disk")]
    WriteFile,
    #[error("server was unable to parse some data")]
    Deserialize,
    #[error("server could not create a directory")]
    Directory,
    #[error("server failed to read requested data from disk")]
    ReadFile,
    #[error("server could not serialize some data")]
    Serialize,
    #[error("server was unable to delete the data")]
    DeleteFailed,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("user is muted")]
    Muted,
    #[error("user is banned")]
    Banned,
    #[error("hub does not exist")]
    HubNotFound,
    #[error("channel does not exist")]
    ChannelNotFound,
    #[error("user does not have the {0} hub permission")]
    MissingHubPermission(HubPermission),
    #[error("user does not have the {0} channel permission")]
    MissingChannelPermission(ChannelPermission),
    #[error("user not in hub")]
    NotInHub,
    #[error("user does not exist")]
    UserNotFound,
    #[error("member does not exist")]
    MemberNotFound,
    #[error("message does not exist")]
    MessageNotFound,
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("group does not exist")]
    GroupNotFound,
    #[error("name is not valid, too long or invalid characters")]
    InvalidName,
    #[error("server did something unexpected")]
    UnexpectedServerArg,
    #[error("message is too big, maximum is {} bytes", MESSAGE_MAX_SIZE)]
    MessageTooBig,
    #[error("unable to parse message, only UTF-8 is supported")]
    InvalidMessage,
    #[error("{0}")]
    Other(String, StatusCode),
    #[error(transparent)]
    Auth(#[from] auth::AuthError),
    #[error(transparent)]
    Data(#[from] DataError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
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
            ApiError::InvalidMessage => Self::BAD_REQUEST,
            ApiError::Other(_, code) => code.clone(),
            ApiError::Auth(error) => error.into(),
            ApiError::Data(_) => Self::INTERNAL_SERVER_ERROR,
            ApiError::Io(_) => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

pub type Result<T, E = crate::ApiError> = std::result::Result<T, E>;

pub const USER_AGENT_STRING: &str = concat!("WICRS Server ", env!("CARGO_PKG_VERSION"));
pub const NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
pub const MESSAGE_MAX_SIZE: usize = 4096;

lazy_static! {
    static ref AUTH: Arc<Mutex<Auth>> = Arc::new(Mutex::new(Auth::from_config()));
    pub static ref CONFIG: config::Config = config::load_config();
}

pub async fn start(bind_address: &str) -> std::io::Result<()> {
    httpapi::server(bind_address).await
}

pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub fn is_valid_name(name: &str) -> bool {
    name.len() > 0 && name.len() < 32 && name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c))
}

pub fn check_name_validity(name: &str) -> Result<()> {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(ApiError::InvalidName)
    }
}

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

pub type ID = Uuid;
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}

pub mod prelude {
    pub use crate::api::*;
    pub use crate::auth::{IDToken, Service};
    pub use crate::channel::{Channel, Message};
    pub use crate::check_name_validity;
    pub use crate::hub::{Hub, HubMember, PermissionGroup};
    pub use crate::is_valid_name;
    pub use crate::new_id;
    pub use crate::permission::{
        ChannelPermission, ChannelPermissions, HubPermission, HubPermissions, PermissionSetting,
    };
    pub use crate::user::{get_id, GenericUser, User};
    pub use crate::ApiError;
    pub use crate::Result;
    pub use crate::ID;
    pub use crate::MESSAGE_MAX_SIZE;
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
        assert!(is_valid_name(""));
        assert!(is_valid_name("Test! @thing"));
        assert!(is_valid_name("123456789111315171921232527293133"));
    }
}
