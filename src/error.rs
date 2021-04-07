use crate::permission::{ChannelPermission, HubPermission};
use parse_display::{Display, FromStr};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors related to data processing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum DataError {
    WriteFile,
    Deserialize,
    Directory,
    ReadFile,
    Serialize,
    DeleteFailed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum IndexError {
    OpenCreateIndex,
    CreateReader,
    CreateWriter,
    GetReader,
    GetWriter,
    ParseQuery,
    Search,
    GetDoc,
    Commit,
}

/// Errors related to authentication.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum AuthError {
    NoResponse,
    BadJson,
    OAuthExchangeFailed,
    InvalidToken,
    InvalidSession,
    MalformedIDToken,
}

impl From<&AuthError> for StatusCode {
    fn from(error: &AuthError) -> Self {
        match error {
            AuthError::InvalidToken => Self::UNAUTHORIZED,
            AuthError::MalformedIDToken => Self::BAD_REQUEST,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
}

/// General errors that can occur when using the WICRS API.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum Error {
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
    AlreadyTyping,
    NotTyping,
    InternalMessageFailed,
    Io,
    #[display("{}({0})")]
    Auth(AuthError),
    #[display("{}({0})")]
    Data(DataError),
    #[display("{}({0})")]
    Index(IndexError),
}

impl From<IndexError> for Error {
    fn from(err: IndexError) -> Self {
        Self::Index(err)
    }
}

impl From<AuthError> for Error {
    fn from(err: AuthError) -> Self {
        Self::Auth(err)
    }
}

impl From<DataError> for Error {
    fn from(err: DataError) -> Self {
        Self::Data(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

impl From<&Error> for StatusCode {
    fn from(error: &Error) -> Self {
        match error {
            Error::NotAuthenticated => Self::UNAUTHORIZED,
            Error::InvalidName => Self::BAD_REQUEST,
            Error::Banned => Self::FORBIDDEN,
            Error::ChannelNotFound => Self::NOT_FOUND,
            Error::GroupNotFound => Self::NOT_FOUND,
            Error::HubNotFound => Self::NOT_FOUND,
            Error::MemberNotFound => Self::NOT_FOUND,
            Error::MessageNotFound => Self::NOT_FOUND,
            Error::Muted => Self::FORBIDDEN,
            Error::MissingChannelPermission(_) => Self::FORBIDDEN,
            Error::MissingHubPermission(_) => Self::FORBIDDEN,
            Error::NotInHub => Self::NOT_FOUND,
            Error::UserNotFound => Self::NOT_FOUND,
            Error::UnexpectedServerArg => Self::INTERNAL_SERVER_ERROR,
            Error::MessageTooBig => Self::BAD_REQUEST,
            Error::CannotAuthenticate => Self::INTERNAL_SERVER_ERROR,
            Error::InvalidMessage => Self::BAD_REQUEST,
            Error::MessageSendFailed => Self::INTERNAL_SERVER_ERROR,
            Error::AlreadyTyping => Self::CONFLICT,
            Error::NotTyping => Self::CONFLICT,
            Error::InternalMessageFailed => Self::INTERNAL_SERVER_ERROR,
            Error::Auth(error) => error.into(),
            Error::Data(_) => Self::INTERNAL_SERVER_ERROR,
            Error::Index(_) => Self::INTERNAL_SERVER_ERROR,
            Error::Io => Self::INTERNAL_SERVER_ERROR,
        }
    }
}
