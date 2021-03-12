use std::fmt::Write;

use crate::{
    auth::{Auth, AuthQuery, Service},
    hub::{Hub, HUB_DATA_FOLDER},
    permission::HubPermission,
    user::User,
    Error, Result, AUTH, ID,
};

use actix_web::{
    delete,
    dev::Payload,
    error, get, post, put,
    web::{Json, Path, Query},
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError,
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
            .service(get_hub_member)
            .service(join_hub)
            .service(leave_hub)
            .service(kick_user)
            .service(ban_user)
            .service(unban_user)
            .service(mute_user)
            .service(unmute_user)
            .service(create_channel)
    })
    .bind(bind_address)?
    .run()
    .await
}

impl ResponseError for Error {
    fn status_code(&self) -> reqwest::StatusCode {
        self.http_status_code()
    }

    fn error_response(&self) -> HttpResponse {
        let mut resp = HttpResponse::new(self.status_code());
        let mut buf = actix_web::web::BytesMut::new();
        let _ = buf.write_str(self.info_string());
        resp.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            actix_web::http::HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        resp.set_body(actix_web::dev::Body::from(buf))
    }
}

impl FromRequest for User {
    type Error = actix_web::Error;

    type Future = Ready<actix_web::Result<Self>>;

    type Config = ();

    fn from_request(request: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let result = futures::executor::block_on(async {
            if let Some(header) = request.headers().get_all("Authorization").next() {
                if let Ok(header_str) = header.to_str() {
                    if let Some(encoded) = header_str.trim().strip_prefix("Basic") {
                        let mut result = String::new();
                        if let Ok(decoded) = base64::decode(encoded.trim()) {
                            let _tostring = decoded.as_slice().read_to_string(&mut result).await;
                            if let Some(split) = result.split_once(':') {
                                if let Ok(id) = ID::parse_str(split.0) {
                                    return if Auth::is_authenticated(
                                        crate::AUTH.clone(),
                                        id,
                                        split.1.to_string(),
                                    )
                                    .await
                                    {
                                        if let Ok(user) = Self::load(&id).await {
                                            ok(user)
                                        } else {
                                            err(error::ErrorNotFound("User not found."))
                                        }
                                    } else {
                                        err(error::ErrorUnauthorized("Invalid ID:token pair."))
                                    };
                                }
                            }
                        }
                    }
                }
            }
            err(error::ErrorBadRequest("Malformed request."))
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
async fn login_finish(
    service: Path<Service>,
    query: Query<AuthQuery>,
) -> actix_web::Result<impl Responder> {
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
    crate::api::get_user_stripped(id.0)
        .await
        .and_then(|u| Ok(Json(u)))
}

#[post("/v2/hub/create/{name}")]
async fn create_hub(mut user: User, name: Path<String>) -> Result<impl Responder> {
    crate::api::create_hub(name.0, &mut user)
        .await
        .and_then(|i| Ok(i.to_string()))
}

#[get("/v2/hub/{hub_id}")]
async fn get_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    crate::api::get_hub(&user, hub_id.0)
        .await
        .and_then(|h| Ok(Json(h)))
}

#[delete("/v2/hub/{hub_id}")]
async fn delete_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id) {
        if let Ok(hub) = Hub::load(*hub_id).await {
            if let Some(member) = hub.members.get(&user.id) {
                return if member.has_permission(HubPermission::All, &hub) {
                    if let Ok(_remove) = tokio::fs::remove_file(
                        HUB_DATA_FOLDER.to_owned() + "/" + &hub_id.to_string() + ".json",
                    )
                    .await
                    {
                        Ok("Successfully deleted the hub.")
                    } else {
                        Err(Error::DeleteFailed)
                    }
                } else {
                    Err(Error::NoPermission)
                };
            }
        }
    }
    Err(Error::HubNotFound)
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
                return if member.has_permission(HubPermission::Administrate, &hub) {
                    let old_name = hub.name;
                    hub.name = query.name.clone();
                    hub.save().await?;
                    Ok(old_name)
                } else {
                    Err(Error::NoPermission)
                };
            }
        }
    }
    Err(Error::HubNotFound)
}

#[get("/v2/hub/{hub_id}/is_banned/{user_id}")]
async fn is_banned_from_hub(
    _user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    if let Ok(hub) = Hub::load(*hub_id).await {
        if hub.bans.contains(&user_id.0) {
            return Ok("true");
        }
    }
    Ok("false")
}

#[get("/v2/hub/{hub_id}/is_muted/{user_id}")]
async fn hub_member_is_muted(
    _user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    if let Ok(hub) = Hub::load(*hub_id).await {
        if hub.mutes.contains(&user_id.0) {
            return Ok("true");
        }
    }
    Ok("false")
}

#[get("/v2/hub/{hub_id}/{user_id}")]
async fn get_hub_member(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id.0) {
        if let Ok(hub) = Hub::load(*hub_id).await {
            if let Some(member) = hub.members.get(&user_id) {
                return Ok(Json(member.clone()));
            }
        }
    }
    Err(Error::HubNotFound)
}

#[post("/v2/hub/join/{hub_id}")]
async fn join_hub(mut user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    if let Ok(member) = user.join_hub(hub_id.0).await {
        Ok(Json(member))
    } else {
        Err(Error::HubNotFound)
    }
}

#[post("/v2/hub/leave/{hub_id}")]
async fn leave_hub(mut user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    if let Ok(()) = user.leave_hub(hub_id.0).await {
        Ok("")
    } else {
        Err(Error::HubNotFound)
    }
}

async fn hub_user_op(
    user: User,
    hub_id: ID,
    user_id: ID,
    op: HubPermission,
) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id) {
        if let Ok(mut hub) = Hub::load(hub_id).await {
            if let Some(member) = hub.members.get(&user.id) {
                return if member.has_permission(op.clone(), &hub) {
                    match op {
                        HubPermission::Kick => hub
                            .kick_user(user_id)
                            .await
                            .and_then(|_| Ok("User kicked.")),
                        HubPermission::Ban => {
                            hub.ban_user(user_id).await.and_then(|_| Ok("User banned."))
                        }
                        HubPermission::Unban => {
                            hub.bans.remove(&user_id);
                            hub.save().await.and_then(|_| Ok("User unbanned."))
                        }
                        HubPermission::Mute => {
                            hub.mutes.insert(user_id);
                            hub.save().await.and_then(|_| Ok("User muted."))
                        }
                        HubPermission::Unmute => {
                            hub.mutes.remove(&user_id);
                            hub.save().await.and_then(|_| Ok("User unmuted."))
                        }
                        _ => Err(Error::UnexpectedServerArg),
                    }
                } else {
                    Err(Error::NoPermission)
                };
            }
        }
    }
    Err(Error::HubNotFound)
}

#[post("/v2/hub/{hub_id}/{user_id}/kick")]
async fn kick_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    hub_user_op(user, hub_id.0, user_id.0, HubPermission::Kick).await
}

#[post("/v2/hub/{hub_id}/{user_id}/ban")]
async fn ban_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    hub_user_op(user, hub_id.0, user_id.0, HubPermission::Ban).await
}

#[post("/v2/hub/{hub_id}/{user_id}/unban")]
async fn unban_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    hub_user_op(user, hub_id.0, user_id.0, HubPermission::Unban).await
}

#[post("/v2/hub/{hub_id}/{user_id}/mute")]
async fn mute_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    hub_user_op(user, hub_id.0, user_id.0, HubPermission::Mute).await
}

#[post("/v2/hub/{hub_id}/{user_id}/unmute")]
async fn unmute_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    hub_user_op(user, hub_id.0, user_id.0, HubPermission::Unmute).await
}

#[post("/v2/channel/create/{hub_id}/{name}")]
async fn create_channel(
    user: User,
    hub_id: Path<ID>,
    name: Path<String>,
) -> Result<impl Responder> {
    if user.in_hubs.contains(&hub_id.0) {
        if let Ok(mut hub) = Hub::load(hub_id.0).await {
            return hub
                .new_channel(user.id.clone(), name.0)
                .await
                .and_then(|channel_id| Ok(channel_id.to_string()));
        }
    }
    Err(Error::HubNotFound)
}
