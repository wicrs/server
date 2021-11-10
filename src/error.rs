use std::string::FromUtf8Error;

use crate::permission::{ChannelPermission, HubPermission};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// General result type for wicrs, error type defaults to [`Error`].
#[cfg(feature = "server")]
pub type Result<T = (), E = Error> = std::result::Result<T, E>;
/// General result type for wicrs public api, error type defaults to [`ApiError`].
pub type ApiResult<T = (), E = ApiError> = std::result::Result<T, E>;

/// General errors that can occur when using the WICRS API.
#[cfg(feature = "server")]
#[derive(Debug, Error)]
pub enum Error {
    #[error("internal handler servers failed to start")]
    ServerStartFailed,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Bincode(#[from] bincode::Error),
    #[error(transparent)]
    Tantivy(#[from] tantivy::error::TantivyError),
    #[error(transparent)]
    TantivyOpenDirectory(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error(transparent)]
    TantivyOpenRead(#[from] tantivy::directory::error::OpenReadError),
    #[error(transparent)]
    TantivyOpenWrite(#[from] tantivy::directory::error::OpenWriteError),
    #[error(transparent)]
    TantivyQueryParse(#[from] tantivy::query::QueryParserError),
    #[error("could not get a Tantivy index writer")]
    GetIndexWriter,
    #[error("could not get a Tantivy index reader")]
    GetIndexReader,
    #[error(transparent)]
    Warp(#[from] warp::Error),
    #[error("could not parse ID")]
    Id(#[from] uuid::Error),
    #[error(transparent)]
    Http(#[from] warp::http::Error),
    #[error(transparent)]
    ApiError(#[from] ApiError),
    #[error("{0}")]
    OtherInternal(String),
}

#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum ApiError {
    #[error("user is muted and cannot send messages")]
    Muted,
    #[error("user is banned from that hub")]
    Banned,
    #[error("hub does not exist")]
    HubNotFound,
    #[error("channel does not exist")]
    ChannelNotFound,
    #[error("user does not have the \"{permission}\" hub permission")]
    MissingHubPermission { permission: HubPermission },
    #[error("user does not have the \"{permission}\" channel permission")]
    MissingChannelPermission { permission: ChannelPermission },
    #[error("user is not in the hub")]
    NotInHub,
    #[error("member does not exist")]
    MemberNotFound,
    #[error("message does not exist")]
    MessageNotFound,
    #[error("permission group does not exist")]
    GroupNotFound,
    #[error("invalid name")]
    InvalidName,
    #[error("not authenticated for websocket")]
    WsNotAuthenticated,
    #[error("text object to big")]
    TooBig,
    #[error("invalid timestamp")]
    InvalidTime,
    #[error("text must use UTF-8 encoding")]
    InvalidText,
    #[error("user already typing")]
    AlreadyTyping,
    #[error("user not typing")]
    NotTyping,
    #[error("something bad happened server-side")]
    InternalError,
    #[error("invalid id")]
    Id,
    #[error("bad request: {message}")]
    Http { message: String },
    #[error("invalid json: {message}")]
    Json { message: String },
    #[error("{message}")]
    Other { message: String },
}

#[cfg(feature = "server")]
impl From<&Error> for ApiError {
    fn from(e: &Error) -> Self {
        match e {
            Error::Json(error) => Self::Json {
                message: error.to_string(),
            },
            Error::Id(_) => Self::Id,
            Error::Http(error) => Self::Http {
                message: error.to_string(),
            },
            Error::ApiError(error) => error.to_owned(),
            _ => Self::InternalError,
        }
    }
}

#[cfg(feature = "server")]
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::OtherInternal(s)
    }
}

impl From<String> for ApiError {
    fn from(s: String) -> Self {
        Self::Other { message: s }
    }
}

impl From<FromUtf8Error> for ApiError {
    fn from(_: FromUtf8Error) -> Self {
        Self::InvalidText
    }
}
