use std::str::FromStr;

use crate::{
    auth::{Auth, AuthQuery, Service},
    is_valid_username,
    user::User,
    AUTH, ID,
};

use actix_web::{
    delete,
    dev::Payload,
    error, get, post, put,
    web::{Json, Path, Query},
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use futures::{
    future::{err, ok, Ready},
    AsyncReadExt,
};
use serde::Deserialize;

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

#[post("/v2/invalidate_tokens")]
async fn invalidate_tokens(user: User) -> impl Responder {
    Auth::invalidate_tokens(AUTH.clone(), &user.id).await;
    "All authentication tokens for your account have been invalidated."
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

#[derive(Deserialize)]
struct IsBotAccount {
    bot: Option<bool>,
}

#[post("/v2/user/account/create/{name}")]
async fn create_account(
    mut user: User,
    name: Path<String>,
    query: Query<IsBotAccount>,
) -> Result<impl Responder> {
    if is_valid_username(&name.0) {
        if let Ok(new_account) = user
            .create_new_account(name.0, query.0.bot.unwrap_or(false))
            .await
        {
            Ok(Json(new_account))
        } else {
            Err(error::ErrorInternalServerError(
                "Failed to save new account data.",
            ))
        }
    } else {
        Err(error::ErrorBadRequest("Malformed request."))
    }
}

#[delete("/v2/user/account/{id}/delete")]
async fn delete_account(mut user: User, id: Path<String>) -> Result<impl Responder> {
    if let Ok(id) = ID::from_str(&id) {
        if let Some(_removed) = user.accounts.remove(&id) {
            if let Ok(()) = user.save().await {
                Ok("Account has been removed.")
            } else {
                Err(error::ErrorInternalServerError(
                    "Unable to remove the account.",
                ))
            }
        } else {
            Err(error::ErrorNotFound("No account with that ID."))
        }
    } else {
        Err(error::ErrorBadRequest("Malformed request."))
    }
}

#[derive(Deserialize)]
struct AccountName {
    name: String,
}

#[put("/v2/user/account/{account_id}/rename")]
async fn rename_account(
    mut user: User,
    account_id: Path<String>,
    query: Query<AccountName>,
) -> Result<impl Responder> {
    if is_valid_username(&query.0.name) {
        if let Ok(id) = ID::from_str(&account_id) {
            if let Some(account) = user.accounts.get_mut(&id) {
                let old_name = account.username.clone();
                account.username = query.0.name;
                Ok(old_name)
            } else {
                Err(error::ErrorNotFound("No account with that ID."))
            }
        } else {
            Err(error::ErrorBadRequest("Malformed request."))
        }
    } else {
        Err(error::ErrorBadRequest("Malformed request."))
    }
}
