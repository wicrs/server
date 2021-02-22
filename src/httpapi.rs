use std::str::FromStr;

use crate::{
    auth::{Auth, AuthQuery, Service},
    user::User,
    AUTH, ID,
};

use actix_web::{
    dev::Payload,
    error, get,
    web::{Json, Path, Query},
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use futures::{
    future::{err, ok, Ready},
    AsyncReadExt,
};

pub(crate) async fn server(bind_address: &str) -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(login_start)
            .service(login_finish)
            .service(get_user)
            .service(get_user_by_id)
            .service(get_account)
            .service(get_user_account)
    })
    .bind(bind_address)?
    .run()
    .await
}

impl FromRequest for User {
    type Error = actix_web::Error;

    type Future = Ready<actix_web::Result<Self>>;

    type Config = ();

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let result = futures::executor::block_on(async {
            let bad_request_error = err(error::ErrorBadRequest("Malformed request."));
            if let Some(header) = request.headers().get_all("Authorization").next() {
                if let Ok(header_str) = header.to_str() {
                    if let Some(encoded) = header_str.trim().strip_prefix("Basic") {
                        let mut result = String::new();
                        if let Ok(decoded) = base64::decode(encoded.trim()) {
                            let _tostring = decoded.as_slice().read_to_string(&mut result).await;
                            if let Some(split) = result.split_once(':') {
                                if Auth::is_authenticated(
                                    crate::AUTH.clone(),
                                    split.0,
                                    split.1.to_string(),
                                )
                                .await
                                {
                                    if let Ok(user) = Self::load(split.0).await {
                                        ok(user)
                                    } else {
                                        err(error::ErrorNotFound("No user with that ID."))
                                    }
                                } else {
                                    err(error::ErrorUnauthorized("Invalid ID:token pair."))
                                }
                            } else {
                                bad_request_error
                            }
                        } else {
                            bad_request_error
                        }
                    } else {
                        bad_request_error
                    }
                } else {
                    bad_request_error
                }
            } else {
                bad_request_error
            }
        });
        result
    }

    fn extract(req: &actix_web::HttpRequest) -> Self::Future {
        Self::from_request(req, &mut actix_web::dev::Payload::None)
    }

    fn configure<F>(f: F) -> Self::Config
    where
        F: FnOnce(Self::Config) -> Self::Config,
    {
        f(Self::Config::default())
    }
}

#[get("/")]
async fn index() -> impl Responder {
    "WICRS is up and running!"
}

#[get("/v2/login/{service}")]
async fn login_start(service: Path<Service>) -> HttpResponse {
    let uri = Auth::start_login(AUTH.clone(), service.0).await;
    HttpResponse::Found().header("Location", uri).finish()
}

#[get("/v2/auth/{service}")]
async fn login_finish(service: Path<Service>, query: Query<AuthQuery>) -> Result<impl Responder> {
    let result = Auth::handle_oauth(AUTH.clone(), service.0, query.0).await;
    if let Some((id, token)) = result.1 {
        Ok(Json(serde_json::json!({
            "id": id,
            "token": token
        })))
    } else {
        Err(error::ErrorBadRequest("Invalid OAuth session."))
    }
}

#[get("/v2/user")]
async fn get_user(user: User) -> impl Responder {
    Json(user)
}

#[get("/v2/user/{id}")]
async fn get_user_by_id(_user: User, id: Path<String>) -> Result<impl Responder> {
    if let Ok(other) = User::load(id.0.as_str()).await {
        Ok(Json(other.to_generic()))
    } else {
        Err(error::ErrorNotFound("User not found."))
    }
}

#[get("/v2/user/account/{account_id}")]
async fn get_account(user: User, account_id: Path<String>) -> Result<impl Responder> {
    if let Ok(id) = ID::from_str(&account_id) {
        if let Some(account) = user.accounts.get(&id) {
            Ok(Json(account.clone()))
        } else {
            Err(error::ErrorNotFound("No account with that ID."))
        }
    } else {
        Err(error::ErrorBadRequest("Malformed request."))
    }
}

#[get("/v2/user/{user_id}/account/{account_id}")]
async fn get_user_account(
    _user: User,
    user_id: Path<String>,
    account_id: Path<String>,
) -> Result<impl Responder> {
    if let Ok(other) = User::load(user_id.0.as_str()).await {
        if let Ok(id) = ID::from_str(&account_id) {
            if let Some(account) = other.accounts.get(&id) {
                Ok(Json(account.clone()))
            } else {
                Err(error::ErrorNotFound("No account with that ID."))
            }
        } else {
            Err(error::ErrorBadRequest("Malformed request."))
        }
    } else {
        Err(error::ErrorNotFound("User not found."))
    }
}
