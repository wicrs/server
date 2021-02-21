#![feature(proc_macro_hygiene, decl_macro)]
#![feature(str_split_once)]
#![feature(in_band_lifetimes)]

use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use auth::Auth;
use futures::lock::Mutex;

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
#[macro_use]
extern crate serial_test;

pub mod auth;
pub mod channel;
pub mod config;
pub mod httpapi;
pub mod hub;
pub mod permission;
pub mod user;

use uuid::Uuid;

#[derive(Eq, PartialEq, Debug)]
pub enum JsonLoadError {
    ReadFile,
    Deserialize,
}

#[derive(Eq, PartialEq, Debug)]
pub enum JsonSaveError {
    WriteFile,
    Serialize,
    Directory,
}

#[derive(Debug)]
pub enum ApiActionError {
    HubNotFound,
    ChannelNotFound,
    NoPermission,
    NotInHub,
    WriteFileError,
    OpenFileError,
    UserNotFound,
    BadAuth,
    BadNameCharacters,
}

pub static USER_AGENT_STRING: &str = concat!("WICRS Server ", env!("CARGO_PKG_VERSION"));
pub const NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

lazy_static! {
    static ref AUTH: Arc<Mutex<Auth>> = Arc::new(Mutex::new(Auth::from_config()));
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

pub fn is_valid_username(name: &str) -> bool {
    name.len() > 0 && name.len() < 32 && name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c))
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
        assert!(is_valid_username("a"));
        assert!(is_valid_username("Test_test tHAt-tester."));
        assert!(is_valid_username("1234567890"));
        assert!(is_valid_username("l33t 5p34k"));
        assert!(!is_valid_username(""));
        assert!(!is_valid_username("Test! @thing"));
        assert!(!is_valid_username("123456789111315171921232527293133"));
    }
}
