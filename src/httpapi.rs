use std::fmt::Write;

use serde::Deserialize;

use crate::{
    api,
    auth::{Auth, AuthError, AuthQuery, Service},
    channel::{Channel, Message},
    get_system_millis,
    user::User,
    ApiError, Result, ID,
};

use actix_web::{
    delete, get,
    http::header,
    post, put,
    web::{Bytes, Json, Path, Query},
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use futures::future::{err, ok, Ready};

pub(crate) async fn server(bind_address: &str) -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(login_start)
            .service(login_finish)
            .service(invalidate_tokens)
            .service(get_user)
            .service(get_user_by_id)
            .service(rename_user)
            .service(create_hub)
            .service(get_hub)
            .service(rename_hub)
            .service(delete_hub)
            .service(is_banned_from_hub)
            .service(hub_member_is_muted)
            .service(get_hub_member)
            .service(join_hub)
            .service(leave_hub)
            .service(kick_user)
            .service(ban_user)
            .service(unban_user)
            .service(mute_user)
            .service(unmute_user)
            .service(change_nickname)
            .service(create_channel)
            .service(get_channel)
            .service(rename_channel)
            .service(delete_channel)
            .service(send_message)
    })
    .bind(bind_address)?
    .run()
    .await
}

impl ResponseError for ApiError {
    fn status_code(&self) -> reqwest::StatusCode {
        self.into()
    }

    fn error_response(&self) -> HttpResponse {
        let mut resp = HttpResponse::new(self.status_code());
        let mut buf = actix_web::web::BytesMut::new();
        let _ = buf.write_fmt(format_args!("{}", self));
        resp.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            actix_web::http::HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        resp.set_body(actix_web::dev::Body::from(buf))
    }
}

impl FromRequest for User {
    type Error = ApiError;

    type Future = Ready<Result<Self>>;

    type Config = ();

    fn from_request(request: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let result = futures::executor::block_on(async {
            if let Some(header) = request.headers().get(header::AUTHORIZATION) {
                if let Ok(header_str) = header.to_str() {
                    dbg!(header_str);
                    if let Some(split) = header_str.split_once(':') {
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
                                    err(ApiError::UserNotFound)
                                }
                            } else {
                                err(AuthError::InvalidToken.into())
                            };
                        }
                    }
                }
            }
            err(AuthError::MalformedIDToken.into())
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
        $op.and_then(|_| Ok(HttpResponse::NoContent().finish()))
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
async fn index() -> String {
    if crate::CONFIG.show_version {
        format!(
            "WICRS server version {} is up and running!",
            env!("CARGO_PKG_VERSION")
        )
    } else {
        String::from("WICRS server is up and running!")
    }
}

#[get("/v2/login/{service}")]
async fn login_start(service: Path<Service>) -> HttpResponse {
    HttpResponse::Found()
        .header("Location", api::start_login(service.0).await)
        .finish()
}

#[get("/v2/auth/{service}")]
async fn login_finish(
    service: Path<Service>,
    query: Query<AuthQuery>,
) -> Result<Json<crate::auth::IDToken>> {
    json_response!(api::complete_login(service.0, query.0).await)
}

#[post("/v2/invalidate_tokens")]
async fn invalidate_tokens(user: User) -> HttpResponse {
    api::invalidate_tokens(&user).await;
    HttpResponse::NoContent().finish()
}

#[get("/v2/user")]
async fn get_user(user: User) -> Json<User> {
    Json(user)
}

#[get("/v2/user/{id}")]
async fn get_user_by_id(_user: User, id: Path<ID>) -> Result<Json<crate::user::GenericUser>> {
    json_response!(api::get_user_stripped(id.0).await)
}

#[put("/v2/user/change_username/{name}")]
async fn rename_user(mut user: User, name: Path<String>) -> Result<String> {
    api::change_username(&mut user, name.0).await
}

#[post("/v2/hub/create/{name}")]
async fn create_hub(mut user: User, name: Path<String>) -> Result<String> {
    string_response!(api::create_hub(&mut user, name.0).await)
}

#[get("/v2/hub/{hub_id}")]
async fn get_hub(user: User, hub_id: Path<ID>) -> Result<Json<crate::hub::Hub>> {
    json_response!(api::get_hub(&user, &hub_id.0).await)
}

#[delete("/v2/hub/{hub_id}")]
async fn delete_hub(user: User, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::delete_hub(&user, &hub_id.0).await)
}

#[put("/v2/hub/rename/{hub_id}/{name}")]
async fn rename_hub(user: User, path: Path<(ID, String)>) -> Result<String> {
    api::rename_hub(&user, &path.0 .0, path.1.clone()).await
}

#[get("/v2/member/{hub_id}/{user_id}/is_banned")]
async fn is_banned_from_hub(user: User, path: Path<(ID, ID)>) -> Result<String> {
    string_response!(api::user_banned(&user, &path.0 .0, &path.1).await)
}

#[get("/v2/member/{hub_id}/{user_id}/is_muted")]
async fn hub_member_is_muted(user: User, path: Path<(ID, ID)>) -> Result<String> {
    string_response!(api::user_muted(&user, &path.0 .0, &path.1).await)
}

#[get("/v2/hub/{hub_id}/{user_id}")]
async fn get_hub_member(user: User, path: Path<(ID, ID)>) -> Result<Json<crate::hub::HubMember>> {
    json_response!(api::get_hub_member(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/hub/join/{hub_id}")]
async fn join_hub(mut user: User, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::join_hub(&mut user, &hub_id.0).await)
}

#[post("/v2/hub/leave/{hub_id}")]
async fn leave_hub(mut user: User, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::leave_hub(&mut user, &hub_id.0).await)
}

#[post("/v2/member/{hub_id}/{user_id}/kick")]
async fn kick_user(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::kick_user(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/ban")]
async fn ban_user(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::ban_user(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/unban")]
async fn unban_user(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::unban_user(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/mute")]
async fn mute_user(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::mute_user(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/unmute")]
async fn unmute_user(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::unmute_user(&user, &path.0 .0, &path.1).await)
}

#[put("/v2/member/change_nickname/{hub_id}/{name}")]
async fn change_nickname(user: User, path: Path<(ID, String)>) -> Result<String> {
    api::change_nickname(&user, &path.0 .0, path.1.clone()).await
}

#[post("/v2/channel/create/{hub_id}/{name}")]
async fn create_channel(user: User, path: Path<(ID, String)>) -> Result<String> {
    string_response!(api::create_channel(&user, &path.0 .0, path.1.clone()).await)
}

#[get("/v2/channel/{hub_id}/{channel_id}")]
async fn get_channel(user: User, path: Path<(ID, ID)>) -> Result<Json<Channel>> {
    json_response!(api::get_channel(&user, &path.0 .0, &path.1).await)
}

#[put("/v2/channel/rename/{hub_id}/{channel_id}/{name}")]
async fn rename_channel(user: User, path: Path<(ID, ID, String)>) -> Result<String> {
    api::rename_channel(&user, &path.0 .0, &path.1, path.2.clone()).await
}

#[delete("/v2/channel/delete/{hub_id}/{channel_id}")]
async fn delete_channel(user: User, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::delete_channel(&user, &path.0 .0, &path.1).await)
}

#[post("/v2/message/send/{hub_id}/{channel_id}")]
async fn send_message(user: User, path: Path<(ID, ID)>, message: Bytes) -> Result<String> {
    if let Ok(message) = String::from_utf8(message.to_vec()) {
        string_response!(api::send_message(&user, &path.0 .0, &path.1, message).await)
    } else {
        Err(ApiError::InvalidMessage)
    }
}

#[get("/v2/message/{hub_id}/{channel_id}/{message_id}")]
async fn get_message(user: User, path: Path<(ID, ID, ID)>) -> Result<Json<Message>> {
    json_response!(api::get_message(&user, &path.0 .0, &path.1, &path.2).await)
}

#[derive(Deserialize)]
struct GetMessagesQuery {
    from: Option<u128>,
    to: Option<u128>,
    invert: Option<bool>,
    max: Option<usize>,
}

impl GetMessagesQuery {
    fn from(&self) -> u128 {
        self.from.unwrap_or(get_system_millis() - 86400001)
    }
    fn to(&self) -> u128 {
        self.to.unwrap_or(get_system_millis())
    }
    fn max(&self) -> usize {
        self.max.unwrap_or(100)
    }
    fn invert(&self) -> bool {
        self.invert.unwrap_or(false)
    }
}

#[get("/v2/message/{hub_id}/{channel_id}/get")]
async fn get_messages(
    user: User,
    path: Path<(ID, ID)>,
    query: Query<GetMessagesQuery>,
) -> Result<Json<Vec<Message>>> {
    json_response!(
        api::get_messages(
            &user,
            &path.0 .0,
            &path.1,
            query.from(),
            query.to(),
            query.invert(),
            query.max()
        )
        .await
    )
}
