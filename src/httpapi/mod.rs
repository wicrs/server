use crate::config::Config;
use crate::error::{ApiError, Error, Result};
use crate::server::ServerAddress;
use serde::{Deserialize, Serialize};
use std::marker::Send;
use std::net::SocketAddr;
use warp::http::{HeaderValue, StatusCode};
use warp::reject::Reject;
use warp::Reply;

pub mod handlers;
pub mod routes;

pub async fn start(config: Config, server: ServerAddress) -> Result {
    let http_server = warp::serve(routes::routes(server)).run(
        config
            .address
            .parse::<SocketAddr>()
            .expect("Invalid bind address"),
    );

    http_server.await;
    Ok(())
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Response<T> {
    Success(T),
    Error(ApiError),
}

pub const fn ok() -> Response<&'static str> {
    Response::Success("OK")
}

impl Reject for ApiError {}

impl Reject for Error {}

impl From<&ApiError> for StatusCode {
    fn from(error: &ApiError) -> Self {
        match error {
            ApiError::Banned
            | ApiError::Muted
            | ApiError::WsNotAuthenticated
            | ApiError::MissingChannelPermission { permission: _ }
            | ApiError::MissingHubPermission { permission: _ } => Self::FORBIDDEN,
            ApiError::ChannelNotFound
            | ApiError::GroupNotFound
            | ApiError::MemberNotFound
            | ApiError::MessageNotFound
            | ApiError::HubNotFound
            | ApiError::NotFound
            | ApiError::NotInHub => Self::NOT_FOUND,
            ApiError::Http { message: _ }
            | ApiError::Json { message: _ }
            | ApiError::Id
            | ApiError::InvalidText
            | ApiError::TooBig
            | ApiError::InvalidTime
            | ApiError::InvalidName => Self::BAD_REQUEST,
            ApiError::AlreadyTyping | ApiError::NotTyping => Self::CONFLICT,
            ApiError::InternalError | ApiError::Other { message: _ } => Self::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Reply for ApiError {
    fn into_response(self) -> warp::reply::Response {
        Response::<()>::Error(self).into_response()
    }
}

impl Reply for &Error {
    fn into_response(self) -> warp::reply::Response {
        ApiError::from(self).into_response()
    }
}

impl<T: Send + Serialize> warp::reply::Reply for Response<T> {
    fn into_response(self) -> warp::reply::Response {
        let mut response = warp::reply::Response::new(warp::hyper::Body::from(
            serde_json::to_string(&self).unwrap(),
        ));

        let status = match &self {
            Self::Error(e) => StatusCode::from(e),
            Self::Success(_) => StatusCode::OK,
        };

        *response.status_mut() = status;
        response
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        response
    }
}
