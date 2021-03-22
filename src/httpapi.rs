use std::fmt::{Display, Write};

use serde::Deserialize;

use wicrs_server::{
    api,
    auth::{Auth, AuthError, AuthQuery, IDToken, Service},
    channel::{Channel, Message},
    get_system_millis,
    hub::{Hub, HubMember},
    permission::{ChannelPermission, HubPermission, PermissionSetting},
    user::{GenericUser, User},
    ApiError, ID,
};

use crate::AUTH;

use actix_web::{
    delete, get,
    http::header,
    post, put,
    web::{Bytes, Json, Path, Query},
    App, FromRequest, HttpRequest, HttpResponse, HttpServer, ResponseError,
};
use futures::future::{err, ok, Ready};

/// Function runs starts an HTTP server that allows HTTP clients to interact with the WICRS Server API. `bind_address` is a string representing the address to bind to, for example it could be `"127.0.0.1:8080"`.
pub async fn server(bind_address: &str) -> std::io::Result<()> {
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

#[derive(Debug)]
struct ApiErrorWrapped(ApiError);

impl Display for ApiErrorWrapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl From<ApiError> for ApiErrorWrapped {
    fn from(err: ApiError) -> Self {
        Self(err)
    }
}

type Result<T, E = ApiErrorWrapped> = std::result::Result<T, E>;

struct UserWrapped(User);

impl ResponseError for ApiErrorWrapped {
    fn status_code(&self) -> reqwest::StatusCode {
        (&self.0).into()
    }

    fn error_response(&self) -> HttpResponse {
        let mut resp = HttpResponse::new(self.status_code());
        let mut buf = actix_web::web::BytesMut::new();
        let _ = buf.write_fmt(format_args!("{}", self.0));
        resp.headers_mut().insert(
            reqwest::header::CONTENT_TYPE,
            actix_web::http::HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        resp.set_body(actix_web::dev::Body::from(buf))
    }
}

impl FromRequest for UserWrapped {
    type Error = ApiErrorWrapped;

    type Future = Ready<Result<Self>>;

    type Config = ();

    fn from_request(request: &HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let result = futures::executor::block_on(async {
            if let Some(header) = request.headers().get(header::AUTHORIZATION) {
                if let Ok(header_str) = header.to_str() {
                    if let Some(split) = header_str.split_once(':') {
                        if let Ok(id) = ID::parse_str(split.0) {
                            return if Auth::is_authenticated(AUTH.clone(), id, split.1.to_string())
                                .await
                            {
                                if let Ok(user) = User::load(&id).await {
                                    ok(UserWrapped(user))
                                } else {
                                    err(ApiErrorWrapped(ApiError::UserNotFound))
                                }
                            } else {
                                err(ApiErrorWrapped(AuthError::InvalidToken.into()))
                            };
                        }
                    }
                }
            }
            err(ApiErrorWrapped(AuthError::MalformedIDToken.into()))
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
            .or_else(|e| Err(ApiErrorWrapped(e)))
    };
}

macro_rules! string_response {
    ($op:expr) => {
        $op.and_then(|t| Ok(t.to_string()))
            .or_else(|e| Err(ApiErrorWrapped(e)))
    };
}

macro_rules! json_response {
    ($op:expr) => {
        $op.and_then(|t| Ok(Json(t)))
            .or_else(|e| Err(ApiErrorWrapped(e)))
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
        .header("Location", api::start_login(AUTH.clone(), service.0).await)
        .finish()
}

#[get("/v2/auth/{service}")]
async fn login_finish(service: Path<Service>, query: Query<AuthQuery>) -> Result<Json<IDToken>> {
    json_response!(api::complete_login(AUTH.clone(), service.0, query.0).await)
}

#[post("/v2/invalidate_tokens")]
async fn invalidate_tokens(user: UserWrapped) -> HttpResponse {
    api::invalidate_tokens(AUTH.clone(), &user.0).await;
    HttpResponse::NoContent().finish()
}

#[get("/v2/user")]
async fn get_user(user: UserWrapped) -> Json<User> {
    Json(user.0)
}

#[get("/v2/user/{id}")]
async fn get_user_by_id(user: UserWrapped, id: Path<ID>) -> Result<Json<GenericUser>> {
    json_response!(api::get_user_stripped(&user.0, id.0).await)
}

#[put("/v2/user/change_username/{name}")]
async fn rename_user(mut user: UserWrapped, name: Path<String>) -> Result<String> {
    api::change_username(&mut user.0, name.0)
        .await
        .or_else(|e| Err(ApiErrorWrapped(e)))
}

#[post("/v2/hub/create/{name}")]
async fn create_hub(mut user: UserWrapped, name: Path<String>) -> Result<String> {
    string_response!(api::create_hub(&mut user.0, name.0).await)
}

#[get("/v2/hub/{hub_id}")]
async fn get_hub(user: UserWrapped, hub_id: Path<ID>) -> Result<Json<Hub>> {
    json_response!(api::get_hub(&user.0, &hub_id.0).await)
}

#[delete("/v2/hub/{hub_id}")]
async fn delete_hub(user: UserWrapped, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::delete_hub(&user.0, &hub_id.0).await)
}

#[put("/v2/hub/rename/{hub_id}/{name}")]
async fn rename_hub(user: UserWrapped, path: Path<(ID, String)>) -> Result<String> {
    api::rename_hub(&user.0, &path.0 .0, path.1.clone())
        .await
        .or_else(|e| Err(ApiErrorWrapped(e)))
}

#[get("/v2/member/{hub_id}/{user_id}/is_banned")]
async fn is_banned_from_hub(user: UserWrapped, path: Path<(ID, ID)>) -> Result<String> {
    string_response!(api::user_banned(&user.0, &path.0 .0, &path.1).await)
}

#[get("/v2/member/{hub_id}/{user_id}/is_muted")]
async fn hub_member_is_muted(user: UserWrapped, path: Path<(ID, ID)>) -> Result<String> {
    string_response!(api::user_muted(&user.0, &path.0 .0, &path.1).await)
}

#[get("/v2/hub/{hub_id}/{user_id}")]
async fn get_hub_member(user: UserWrapped, path: Path<(ID, ID)>) -> Result<Json<HubMember>> {
    json_response!(api::get_hub_member(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/hub/join/{hub_id}")]
async fn join_hub(mut user: UserWrapped, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::join_hub(&mut user.0, &hub_id.0).await)
}

#[post("/v2/hub/leave/{hub_id}")]
async fn leave_hub(mut user: UserWrapped, hub_id: Path<ID>) -> Result<HttpResponse> {
    no_content!(api::leave_hub(&mut user.0, &hub_id.0).await)
}

#[post("/v2/member/{hub_id}/{user_id}/kick")]
async fn kick_user(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::kick_user(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/ban")]
async fn ban_user(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::ban_user(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/unban")]
async fn unban_user(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::unban_user(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/mute")]
async fn mute_user(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::mute_user(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/member/{hub_id}/{user_id}/unmute")]
async fn unmute_user(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::unmute_user(&user.0, &path.0 .0, &path.1).await)
}

#[put("/v2/member/change_nickname/{hub_id}/{name}")]
async fn change_nickname(user: UserWrapped, path: Path<(ID, String)>) -> Result<String> {
    api::change_nickname(&user.0, &path.0 .0, path.1.clone())
        .await
        .or_else(|e| Err(ApiErrorWrapped(e)))
}

#[post("/v2/channel/create/{hub_id}/{name}")]
async fn create_channel(user: UserWrapped, path: Path<(ID, String)>) -> Result<String> {
    string_response!(api::create_channel(&user.0, &path.0 .0, path.1.clone()).await)
}

#[get("/v2/channel/{hub_id}/{channel_id}")]
async fn get_channel(user: UserWrapped, path: Path<(ID, ID)>) -> Result<Json<Channel>> {
    json_response!(api::get_channel(&user.0, &path.0 .0, &path.1).await)
}

#[put("/v2/channel/rename/{hub_id}/{channel_id}/{name}")]
async fn rename_channel(user: UserWrapped, path: Path<(ID, ID, String)>) -> Result<String> {
    api::rename_channel(&user.0, &path.0 .0, &path.1, path.2.clone())
        .await
        .or_else(|e| Err(ApiErrorWrapped(e)))
}

#[delete("/v2/channel/delete/{hub_id}/{channel_id}")]
async fn delete_channel(user: UserWrapped, path: Path<(ID, ID)>) -> Result<HttpResponse> {
    no_content!(api::delete_channel(&user.0, &path.0 .0, &path.1).await)
}

#[post("/v2/message/send/{hub_id}/{channel_id}")]
async fn send_message(user: UserWrapped, path: Path<(ID, ID)>, message: Bytes) -> Result<String> {
    if let Ok(message) = String::from_utf8(message.to_vec()) {
        string_response!(api::send_message(&user.0, &path.0 .0, &path.1, message).await)
    } else {
        Err(ApiErrorWrapped(ApiError::InvalidMessage))
    }
}

#[get("/v2/message/{hub_id}/{channel_id}/{message_id}")]
async fn get_message(user: UserWrapped, path: Path<(ID, ID, ID)>) -> Result<Json<Message>> {
    json_response!(api::get_message(&user.0, &path.0 .0, &path.1, &path.2).await)
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
    user: UserWrapped,
    path: Path<(ID, ID)>,
    query: Query<GetMessagesQuery>,
) -> Result<Json<Vec<Message>>> {
    json_response!(
        api::get_messages(
            &user.0,
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

#[derive(Deserialize)]
struct PermissionSettingQuery {
    pub setting: PermissionSetting,
}

#[put("/v2/member/{member_id}/set_hub_permission/{hub_id}/{hub_permission}")]
async fn set_user_hub_permission(
    user: UserWrapped,
    path: Path<(ID, ID, HubPermission)>,
    query: Query<PermissionSettingQuery>,
) -> Result<HttpResponse> {
    no_content!(
        api::set_member_hub_permission(&user.0, &path.1, &path.0 .0, path.2, query.setting).await
    )
}

#[put("/v2/member/{member_id}/set_hub_permission/{hub_id}/{channel_id}/{channel_permission}")]
async fn set_user_channel_permission(
    user: UserWrapped,
    path: Path<(ID, ID, ID, ChannelPermission)>,
    query: Query<PermissionSettingQuery>,
) -> Result<HttpResponse> {
    no_content!(
        api::set_member_channel_permission(
            &user.0,
            &path.1,
            &path.0 .0,
            &path.2,
            path.3,
            query.setting
        )
        .await
    )
}
