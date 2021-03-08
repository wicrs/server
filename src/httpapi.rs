use rayon::prelude::*;
use std::collections::HashMap;

use crate::{
    auth::{Auth, AuthQuery, Service},
    channel::Channel,
    hub::{Hub, HUB_DATA_FOLDER},
    new_id,
    permission::HubPermission,
    user::User,
    ApiActionError, AUTH, ID,
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
            .service(create_hub)
            .service(get_hub)
            .service(rename_hub)
            .service(delete_hub)
            .service(hub_member_is_muted)
            .service(is_banned_from_hub)
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
                                if let Ok(id) = serde_json::from_str::<ID>(split.0) {
                                    if Auth::is_authenticated(
                                        crate::AUTH.clone(),
                                        id,
                                        split.1.to_string(),
                                    )
                                    .await
                                    {
                                        if let Ok(user) = Self::load(&id).await {
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
    Auth::invalidate_tokens(AUTH.clone(), user.id).await;
    "All authentication tokens for your account have been invalidated."
}

#[get("/v2/user")]
async fn get_user(user: User) -> impl Responder {
    Json(user)
}

#[get("/v2/user/{id}")]
async fn get_user_by_id(_user: User, id: Path<ID>) -> Result<impl Responder> {
    if let Ok(other) = User::load(&id).await {
        Ok(Json(other.to_generic()))
    } else {
        Err(error::ErrorNotFound("User not found."))
    }
}

#[post("/v2/hub/create/{name}")]
async fn create_hub(mut user: User, name: Path<String>) -> Result<impl Responder> {
    let new_hub = user.create_hub(name.0, new_id()).await;
    if let Ok(id) = new_hub {
        Ok(id.to_string())
    } else {
        match new_hub {
            Err(ApiActionError::WriteFileError) => Err(error::ErrorInternalServerError(
                "Unable to create the new hub.",
            )),
            Err(ApiActionError::BadNameCharacters) => {
                Err(error::ErrorBadRequest("Malformed request."))
            }
            _ => Err(error::ErrorInternalServerError(
                "Something strange happened...",
            )),
        }
    }
}

#[get("/v2/hub/{hub_id}")]
async fn get_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id) {
        if let Ok(mut hub) = Hub::load(*hub_id).await {
            if let Ok(channels_allowed) = hub.channels(user.id) {
                let mut sending = hub.clone();
                sending.channels = channels_allowed
                    .into_par_iter()
                    .map(|channel| (channel.id, channel))
                    .collect::<HashMap<ID, Channel>>();
                Ok(Json(sending))
            } else {
                Err(error::ErrorNotFound("Account not found."))
            }
        } else {
            Err(error::ErrorInternalServerError("Failed to load hub data."))
        }
    } else {
        Err(error::ErrorNotFound("Hub not found."))
    }
}

#[delete("/v2/hub/{hub_id}")]
async fn delete_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id) {
        if let Ok(hub) = Hub::load(*hub_id).await {
            if let Some(member) = hub.members.get(&user.id) {
                if member.has_permission(HubPermission::All, &hub) {
                    if let Ok(_remove) = tokio::fs::remove_file(
                        HUB_DATA_FOLDER.to_owned() + "/" + &hub_id.to_string() + ".json",
                    )
                    .await
                    {
                        Ok("Successfully deleted the hub.")
                    } else {
                        Err(error::ErrorInternalServerError(
                            "Failed to delete hub data.",
                        ))
                    }
                } else {
                    Err(error::ErrorForbidden(HubPermission::All))
                }
            } else {
                Err(error::ErrorInternalServerError("Failed to load hub data."))
            }
        } else {
            Err(error::ErrorNotFound("Account not found."))
        }
    } else {
        Err(error::ErrorNotFound("Hub not found."))
    }
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

#[put("/v2/hub/rename/{hub_id}")]
async fn rename_hub(user: User, hub_id: Path<ID>, query: Query<Name>) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id) {
        if let Ok(mut hub) = Hub::load(*hub_id).await {
            if let Some(member) = hub.members.get(&user.id) {
                if member.has_permission(HubPermission::Administrate, &hub) {
                    let old_name = hub.name;
                    hub.name = query.name.clone();
                    if let Ok(_save) = hub.save().await {
                        Ok(old_name)
                    } else {
                        Err(error::ErrorInternalServerError("Failed rename hub."))
                    }
                } else {
                    Err(error::ErrorForbidden(HubPermission::Administrate))
                }
            } else {
                Err(error::ErrorInternalServerError("Failed to load hub data."))
            }
        } else {
            Err(error::ErrorNotFound("Account not found."))
        }
    } else {
        Err(error::ErrorNotFound("Hub not found."))
    }
}

#[get("/v2/hub/{hub_id}/is_banned/{user_id}")]
async fn is_banned_from_hub(
    _user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    if let Ok(hub) = Hub::load(*hub_id).await {
        if hub.bans.contains(&user_id.0) {
            Ok("true")
        } else {
            Ok("false")
        }
    } else {
        Err(error::ErrorNotFound("Hub not found."))
    }
}

#[get("/v2/hub/{hub_id}/is_muted/{user_id}")]
async fn hub_member_is_muted(
    _user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    if let Ok(hub) = Hub::load(*hub_id).await {
        if hub.mutes.contains(&user_id.0) {
            Ok("true")
        } else {
            Ok("false")
        }
    } else {
        Err(error::ErrorNotFound("Hub not found."))
    }
}
