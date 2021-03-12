use std::fmt::Write;

use crate::{
    api,
    auth::{Auth, AuthQuery, Service},
    user::User,
    Error, Result, ID,
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

macro_rules! no_content {
    ($op:expr) => {
        $op.and_then(|_| Ok(HttpResponse::NoContent()))
    };
}

macro_rules! string_response {
    ($op:expr) => {
        $op.and_then(|t| Ok(t.to_string()))
    };
}

macro_rules! json_response {
    ($op:expr) => {
        $op.and_then(|t| Ok(Json(t)))
    };
}

#[get("/")]
async fn index() -> impl Responder {
    "WICRS is up and running!"
}

#[get("/v2/login/{service}")]
async fn login_start(service: Path<Service>) -> HttpResponse {
    HttpResponse::Found()
        .header("Location", api::start_login(service.0).await)
        .finish()
}

#[get("/v2/auth/{service}")]
async fn login_finish(service: Path<Service>, query: Query<AuthQuery>) -> Result<impl Responder> {
    json_response!(api::complete_login(service.0, query.0).await)
}

#[post("/v2/invalidate_tokens")]
async fn invalidate_tokens(user: User) -> impl Responder {
    api::invalidate_tokens(&user).await;
    HttpResponse::NoContent()
}

#[get("/v2/user")]
async fn get_user(user: User) -> impl Responder {
    Json(user)
}

#[get("/v2/user/{id}")]
async fn get_user_by_id(_user: User, id: Path<ID>) -> Result<impl Responder> {
    json_response!(api::get_user_stripped(id.0).await)
}

#[post("/v2/hub/create/{name}")]
async fn create_hub(mut user: User, name: Path<String>) -> Result<impl Responder> {
    string_response!(api::create_hub(name.0, &mut user).await)
}

#[get("/v2/hub/{hub_id}")]
async fn get_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    json_response!(api::get_hub(&user, hub_id.0).await)
}

#[delete("/v2/hub/{hub_id}")]
async fn delete_hub(user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::delete_hub(&user, hub_id.0).await)
}

#[put("/v2/hub/rename/{hub_id}/{name}")]
async fn rename_hub(user: User, hub_id: Path<ID>, name: Path<String>) -> Result<impl Responder> {
    api::rename_hub(&user, hub_id.0, name.0).await
}

#[get("/v2/hub/{hub_id}/is_banned/{user_id}")]
async fn is_banned_from_hub(
    user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    string_response!(api::user_banned(&user, hub_id.0, user_id.0).await)
}

#[get("/v2/hub/{hub_id}/is_muted/{user_id}")]
async fn hub_member_is_muted(
    user: User,
    hub_id: Path<ID>,
    user_id: Path<ID>,
) -> Result<impl Responder> {
    string_response!(api::user_muted(&user, hub_id.0, user_id.0).await)
}

#[get("/v2/hub/{hub_id}/{user_id}")]
async fn get_hub_member(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    json_response!(api::get_hub_member(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/hub/join/{hub_id}")]
async fn join_hub(mut user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::join_hub(&mut user, hub_id.0).await)
}

#[post("/v2/hub/leave/{hub_id}")]
async fn leave_hub(mut user: User, hub_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::leave_hub(&mut user, hub_id.0).await)
}

#[post("/v2/hub/{hub_id}/{user_id}/kick")]
async fn kick_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::kick_user(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/hub/{hub_id}/{user_id}/ban")]
async fn ban_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::ban_user(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/hub/{hub_id}/{user_id}/unban")]
async fn unban_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::unban_user(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/hub/{hub_id}/{user_id}/mute")]
async fn mute_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::mute_user(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/hub/{hub_id}/{user_id}/unmute")]
async fn unmute_user(user: User, hub_id: Path<ID>, user_id: Path<ID>) -> Result<impl Responder> {
    no_content!(api::unmute_user(&user, hub_id.0, user_id.0).await)
}

#[post("/v2/channel/create/{hub_id}/{name}")]
async fn create_channel(
    user: User,
    hub_id: Path<ID>,
    name: Path<String>,
) -> Result<impl Responder> {
    string_response!(api::create_channel(&user, hub_id.0, name.0).await)
}
