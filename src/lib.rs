#![feature(proc_macro_hygiene, decl_macro)]
#![feature(str_split_once)]
#![feature(in_band_lifetimes)]

use auth::Auth;
use futures::lock::Mutex;
use prelude::{ChannelPermission, HubPermission};
use reqwest::StatusCode;
use std::{
    fmt::Display,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    Muted,
    Banned,
    HubNotFound,
    ChannelNotFound,
    MissingHubPermission(HubPermission),
    MissingChannelPermission(ChannelPermission),
    NotInHub,
    WriteFile,
    Deserialize,
    Directory,
    ReadFile,
    Serialize,
    UserNotFound,
    MemberNotFound,
    NotAuthenticated,
    GroupNotFound,
    InvalidName,
    DeleteFailed,
    UnexpectedServerArg,
    MessageTooBig,
    InvalidMessage,
    Other(String, StatusCode),
}

impl Error {
    fn info_string(&self) -> &str {
        match self {
            Self::NotAuthenticated => "You are not authenticated.",
            Self::InvalidName => "Invalid name.",
            Self::Banned => "You are banned from that hub.",
            Self::ChannelNotFound => "Channel not found.",
            Self::Deserialize => "Server was unable to deserialize the data. Try again later.",
            Self::Directory => {
                "Server is missing a directory and could not create it. Try again later."
            }
            Self::GroupNotFound => "Group not found.",
            Self::HubNotFound => "Hub not found.",
            Self::MemberNotFound => "Hub member not found.",
            Self::Muted => "You are muted.",
            Self::MissingChannelPermission(_) => {
                "You are missing the channel permission required to do that."
            }
            Self::MissingHubPermission(_) => {
                "You are missing the hub permission required to do that."
            }
            Self::NotInHub => "You are not in that hub.",
            Self::ReadFile => "Server was unable to read the data. Try again later.",
            Self::Serialize => "Server was unable to serialize the data. Try again later.",
            Self::UserNotFound => "User not found.",
            Self::WriteFile => "Server was unable to store the data. Try again later.",
            Self::DeleteFailed => "Server was unable to delete the data.",
            Self::UnexpectedServerArg => "Something strange happened...",
            Self::MessageTooBig => "Message too big.",
            Self::InvalidMessage => "Messages must be sent as UTF-8 strings.",
            Self::Other(message, _) => message,
        }
    }

    fn http_status_code(&self) -> StatusCode {
        match self {
            Self::NotAuthenticated => StatusCode::UNAUTHORIZED,
            Self::InvalidName => StatusCode::BAD_REQUEST,
            Self::Banned => StatusCode::FORBIDDEN,
            Self::ChannelNotFound => StatusCode::NOT_FOUND,
            Self::Deserialize => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Directory => StatusCode::INTERNAL_SERVER_ERROR,
            Self::GroupNotFound => StatusCode::NOT_FOUND,
            Self::HubNotFound => StatusCode::NOT_FOUND,
            Self::MemberNotFound => StatusCode::NOT_FOUND,
            Self::Muted => StatusCode::FORBIDDEN,
            Self::MissingChannelPermission(_) => StatusCode::FORBIDDEN,
            Self::MissingHubPermission(_) => StatusCode::FORBIDDEN,
            Self::NotInHub => StatusCode::NOT_FOUND,
            Self::ReadFile => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Serialize => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::WriteFile => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DeleteFailed => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnexpectedServerArg => StatusCode::INTERNAL_SERVER_ERROR,
            Self::MessageTooBig => StatusCode::BAD_REQUEST,
            Self::InvalidMessage => StatusCode::BAD_REQUEST,
            Self::Other(_, code) => code.clone(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

pub type Result<T, E = crate::Error> = std::result::Result<T, E>;

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
        Err(Error::InvalidName)
    }
}

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
    pub use crate::Error;
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
