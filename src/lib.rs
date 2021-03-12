#![feature(proc_macro_hygiene, decl_macro)]
#![feature(str_split_once)]
#![feature(in_band_lifetimes)]

use auth::Auth;
use futures::lock::Mutex;
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

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Muted,
    Banned,
    HubNotFound,
    ChannelNotFound,
    NoPermission,
    NotInHub,
    WriteFile,
    Deserialize,
    Directory,
    ReadFile,
    Serialize,
    UserNotFound,
    MemberNotFound,
    BadAuth,
    GroupNotFound,
    InvalidName,
    DeleteFailed,
    UnexpectedServerArg,
    AuthError(String, StatusCode),
}

impl Error {
    fn info_string(&self) -> &str {
        match self {
            Self::BadAuth => "You are not authenticated.",
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
            Self::NoPermission => "You do not have permission to do that.",
            Self::NotInHub => "You are not in that hub.",
            Self::ReadFile => "Server was unable to read the data. Try again later.",
            Self::Serialize => "Server was unable to serialize the data. Try again later.",
            Self::UserNotFound => "User not found.",
            Self::WriteFile => "Server was unable to store the data. Try again later.",
            Self::DeleteFailed => "Server was unable to delete the data.",
            Self::UnexpectedServerArg => "Something strange happened...",
            Self::AuthError(message, _) => message.as_str(),
        }
    }

    fn http_status_code(&self) -> StatusCode {
        match self {
            Self::BadAuth => StatusCode::UNAUTHORIZED,
            Self::InvalidName => StatusCode::BAD_REQUEST,
            Self::Banned => StatusCode::FORBIDDEN,
            Self::ChannelNotFound => StatusCode::NOT_FOUND,
            Self::Deserialize => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Directory => StatusCode::INTERNAL_SERVER_ERROR,
            Self::GroupNotFound => StatusCode::NOT_FOUND,
            Self::HubNotFound => StatusCode::NOT_FOUND,
            Self::MemberNotFound => StatusCode::NOT_FOUND,
            Self::Muted => StatusCode::FORBIDDEN,
            Self::NoPermission => StatusCode::FORBIDDEN,
            Self::NotInHub => StatusCode::NOT_FOUND,
            Self::ReadFile => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Serialize => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UserNotFound => StatusCode::NOT_FOUND,
            Self::WriteFile => StatusCode::INTERNAL_SERVER_ERROR,
            Self::DeleteFailed => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnexpectedServerArg => StatusCode::INTERNAL_SERVER_ERROR,
            Self::AuthError(_, code) => code.clone(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_string().as_str())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub static USER_AGENT_STRING: &str = concat!("WICRS Server ", env!("CARGO_PKG_VERSION"));
pub const NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

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

pub fn is_valid_username(name: &str) -> Result<()> {
    if name.len() > 0 && name.len() < 32 && name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
        Ok(())
    } else {
        Err(Error::InvalidName)
    }
}

pub type ID = Uuid;
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}

#[cfg(test)]
mod tests {
    use super::is_valid_username;

    #[test]
    fn valid_username_check() {
        assert!(is_valid_username("a").is_ok());
        assert!(is_valid_username("Test_test tHAt-tester.").is_ok());
        assert!(is_valid_username("1234567890").is_ok());
        assert!(is_valid_username("l33t 5p34k").is_ok());
        assert!(is_valid_username("").is_err());
        assert!(is_valid_username("Test! @thing").is_err());
        assert!(is_valid_username("123456789111315171921232527293133").is_err());
    }
}
