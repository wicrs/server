use crate::permission::{ChannelPermission, HubPermission};
use parse_display::{Display, FromStr};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Error as TungsteniteError;
use warp::reject::Reject;

/// General result type for wicrs, error type defaults to [`Error`].
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

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

impl Reject for DataError {}

/// Errors related to web socket handling.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Display, FromStr)]
#[display(style = "SNAKE_CASE")]
pub enum WebSocketError {
    ConnectionClosed,
    AlreadyClosed,
    Protocol,
    Utf8,
    Tls,
    Io,
    Url,
    Capacity,
    SendQueueFull,
    Http,
    HttpFormat,
}

impl Reject for WebSocketError {}

/// Errors related to message indexing and searching (Tantivy).
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
    Reload,
}

impl Reject for IndexError {}

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

impl Reject for AuthError {}

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
    TooBig,
    InvalidText,
    MessageSendFailed,
    #[display("{}({0})")]
    WebSocket(WebSocketError),
    CannotAuthenticate,
    AlreadyTyping,
    NotTyping,
    InternalMessageFailed,
    ServerStartFailed,
    Io,
    #[display("{}({0})")]
    Auth(AuthError),
    #[display("{}({0})")]
    Data(DataError),
    #[display("{}({0})")]
    Index(IndexError),
}

impl Reject for Error {}

impl From<TungsteniteError> for WebSocketError {
    fn from(err: TungsteniteError) -> Self {
        match err {
            TungsteniteError::ConnectionClosed => Self::ConnectionClosed,
            TungsteniteError::AlreadyClosed => Self::AlreadyClosed,
            TungsteniteError::Io(_) => Self::Io,
            TungsteniteError::Tls(_) => Self::Tls,
            TungsteniteError::Capacity(_) => Self::Capacity,
            TungsteniteError::Protocol(_) => Self::Protocol,
            TungsteniteError::SendQueueFull(_) => Self::SendQueueFull,
            TungsteniteError::Utf8 => Self::Utf8,
            TungsteniteError::Url(_) => Self::Url,
            TungsteniteError::Http(_) => Self::Http,
            TungsteniteError::HttpFormat(_) => Self::HttpFormat,
        }
    }
}

impl From<&WebSocketError> for StatusCode {
    fn from(err: &WebSocketError) -> Self {
        match err {
            WebSocketError::ConnectionClosed => Self::NO_CONTENT,
            WebSocketError::AlreadyClosed => Self::GONE,
            WebSocketError::Capacity => Self::PAYLOAD_TOO_LARGE,
            WebSocketError::SendQueueFull => Self::TOO_MANY_REQUESTS,
            _ => Self::BAD_REQUEST,
        }
    }
}

impl From<IndexError> for Error {
    fn from(err: IndexError) -> Self {
        Self::Index(err)
    }
}

impl From<WebSocketError> for Error {
    fn from(err: WebSocketError) -> Self {
        Self::WebSocket(err)
    }
}

impl From<TungsteniteError> for Error {
    fn from(err: TungsteniteError) -> Self {
        Self::WebSocket(err.into())
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
            Error::ServerStartFailed => Self::INTERNAL_SERVER_ERROR,
            Error::UnexpectedServerArg => Self::INTERNAL_SERVER_ERROR,
            Error::TooBig => Self::BAD_REQUEST,
            Error::CannotAuthenticate => Self::INTERNAL_SERVER_ERROR,
            Error::InvalidText => Self::BAD_REQUEST,
            Error::MessageSendFailed => Self::INTERNAL_SERVER_ERROR,
            Error::AlreadyTyping => Self::CONFLICT,
            Error::NotTyping => Self::CONFLICT,
            Error::InternalMessageFailed => Self::INTERNAL_SERVER_ERROR,
            Error::Auth(error) => error.into(),
            Error::Data(_) => Self::INTERNAL_SERVER_ERROR,
            Error::Index(_) => Self::INTERNAL_SERVER_ERROR,
            Error::WebSocket(error) => error.into(),
            Error::Io => Self::INTERNAL_SERVER_ERROR,
        }
    }
}
