use reqwest::StatusCode;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

use auth::Auth;

use uuid::Uuid;

use warp::{filters::BoxedFilter, Filter, Reply};

#[cfg(test)]
#[macro_use]
extern crate serial_test;

#[macro_use]
pub mod macros;
pub mod auth;
pub mod channel;
pub mod config;
pub mod guild;
pub mod permission;
pub mod user;

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

pub fn unexpected_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Unexpected error.", StatusCode::INTERNAL_SERVER_ERROR).into_response()
}

pub fn bad_auth_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Invalid authentication details.", StatusCode::FORBIDDEN)
        .into_response()
}

pub fn account_not_found_response() -> warp::http::Response<warp::hyper::Body> {
    warp::reply::with_status("Could not find that account.", StatusCode::NOT_FOUND).into_response()
}

#[derive(Debug)]
pub enum ApiActionError {
    GuildNotFound,
    ChannelNotFound,
    NoPermission,
    NotInGuild,
    WriteFileError,
    OpenFileError,
    UserNotFound,
    BadNameCharacters,
}

static USER_AGENT_STRING: &str = "wirc_server";
static NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[tokio::main]
async fn main() {
    let auth = Arc::new(Mutex::new(Auth::from_config().await));
    let api_v1 = v1_api(auth.clone());
    let api = warp::any().and(warp::path("api")).and(api_v1);
    warp::serve(api.or(warp::any().map(|| {
        warp::reply::with_status(
            "Not found. Make sure to check you provided all of the required parameters.",
            StatusCode::NOT_FOUND,
        )
    })))
    .run((
        [0, 0, 0, 0],
        std::env::var("PORT")
            .unwrap_or(config::load_config().port.to_string())
            .parse()
            .unwrap_or(config::load_config().port),
    ))
    .await;
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

fn v1_api(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    let guild_api = warp::path("guilds").and(guild::api_v1(auth_manager.clone()));
    let auth_api = auth::api_v1(auth_manager.clone());
    let user_api = user::api_v1(auth_manager.clone());
    warp::path("v1")
        .and(auth_api.or(user_api).or(guild_api))
        .boxed()
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
