use std::string::FromUtf8Error;

use crate::permission::{ChannelPermission, HubPermission};
use reqwest::StatusCode;
use thiserror::Error;
use warp::reject::Reject;

/// General result type for wicrs, error type defaults to [`Error`].
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

/// General errors that can occur when using the WICRS API.
#[derive(Debug, Error)]
pub enum Error {
    #[error("user is muted and cannot send messages")]
    Muted,
    #[error("user is banned from that hub")]
    Banned,
    #[error("hub does not exist")]
    HubNotFound,
    #[error("channel does not exist")]
    ChannelNotFound,
    #[error("user does not have the \"{0}\" hub permission")]
    MissingHubPermission(HubPermission),
    #[error("user does not have the \"{0}\" channel permission")]
    MissingChannelPermission(ChannelPermission),
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
    #[error("something strange happened")]
    UnexpectedServerArg,
    #[error("text object to big")]
    TooBig,
    #[error("invalid argument/option/value")]
    BadValue,
    #[error("invalid timestamp")]
    InvalidTime,
    #[error("not utf-8 bytes")]
    InvalidText,
    #[error("bad message format")]
    InvalidMessage,
    #[error("user already typing")]
    AlreadyTyping,
    #[error("user not typing")]
    NotTyping,
    #[error("internal server message failed")]
    InternalMessageFailed,
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
    #[error("request expired")]
    Expired,
    #[error("not authenticated for websocket")]
    WsNotAuthenticated,
    #[error(transparent)]
    Warp(#[from] warp::Error),
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Id(#[from] uuid::Error),
    #[error("could not find a pgp public key with that ID")]
    PublicKeyNotFound,
    #[error("invalid PGP fingerprint")]
    InvalidFingerprint,
    #[error(transparent)]
    Http(#[from] warp::http::Error),
    #[error("{0}")]
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(_: FromUtf8Error) -> Self {
        Self::InvalidText
    }
}

impl Reject for Error {}

impl From<&Error> for StatusCode {
    fn from(error: &Error) -> Self {
        match error {
            Error::Banned
            | Error::Muted
            | Error::MissingChannelPermission(_)
            | Error::MissingHubPermission(_) => Self::FORBIDDEN,
            Error::ChannelNotFound
            | Error::GroupNotFound
            | Error::MemberNotFound
            | Error::MessageNotFound
            | Error::NotInHub => Self::NOT_FOUND,
            Error::Id(_)
            | Error::Http(_)
            | Error::InvalidText
            | Error::TooBig
            | Error::InvalidTime
            | Error::BadValue
            | Error::InvalidFingerprint
            | Error::InvalidName => Self::BAD_REQUEST,
            Error::AlreadyTyping | Error::NotTyping => Self::CONFLICT,
            _ => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

impl warp::reply::Reply for Error {
    fn into_response(self) -> warp::reply::Response {
        let mut response = warp::reply::Response::new(warp::hyper::Body::from(self.to_string()));
        *response.status_mut() = (&self).into();
        response
    }
}
