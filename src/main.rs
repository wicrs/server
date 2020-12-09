use reqwest::StatusCode;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

use auth::{Auth, AuthQuery, Service, TokenQuery, UserQuery};
use serde::{Deserialize, Serialize};
use user::Account;

use guild::SendMessageError;

use uuid::Uuid;

use warp::{filters::BoxedFilter, http::Uri, reply::WithStatus, Filter, Rejection, Reply};

pub mod auth;
pub mod channel;
pub mod config;
pub mod guild;
pub mod permission;
pub mod testing;
pub mod user;

#[derive(Eq, PartialEq, Debug)]
pub enum JsonLoadError {
    ReadFile,
    Deserialize,
}

#[derive(Eq, PartialEq)]
pub enum JsonSaveError {
    WriteFile,
    Serialize,
    Directory,
}

static USER_AGENT_STRING: &str = "wirc_server";
static NAME_ALLOWED_CHARS: &str =
    " .,_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[tokio::main]
async fn main() {
    testing::run().await;
    let auth = Arc::new(Mutex::new(Auth::from_config()));
    let api_v1 = v1_api(auth.clone());
    let api = warp::any().and(warp::path("api")).and(api_v1);
    warp::serve(
        api.or(warp::any().map(|| warp::reply::with_status("Not found.", StatusCode::NOT_FOUND))),
    )
    .run(([127, 0, 0, 1], config::load_config().port))
    .await;
}

pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub type ID = Uuid;
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}

#[derive(Deserialize, Serialize)]
struct AccountTokenResponse {
    id: String,
    token: String,
}

#[derive(Deserialize, Serialize)]
struct MessageBody {
    content: String,
}

fn v1_api(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    let auth_auth = auth_manager.clone();
    let auth = warp::get()
        .and(warp::path!("auth" / Service))
        .and(warp::query::<AuthQuery>())
        .and_then(move |service: Service, query: AuthQuery| {
            let tmp_auth = auth_auth.clone();
            async move {
                let result = Auth::handle_oauth(tmp_auth, service, query).await;
                if let Some(id_token) = result.1 {
                    Ok::<_, Rejection>(warp::reply::json(&AccountTokenResponse {
                        id: id_token.0,
                        token: id_token.1,
                    }))
                } else {
                    Ok::<_, Rejection>(warp::reply::json(&result.0))
                }
            }
        });
    let login_auth = auth_manager.clone();
    let login =
        warp::get()
            .and(warp::path!("login" / Service))
            .and_then(move |service: Service| {
                let tmp_auth = login_auth.clone();
                async move {
                    let uri_string = Auth::start_login(tmp_auth, service).await;
                    let uri = uri_string.parse::<Uri>().unwrap();
                    Ok::<_, Rejection>(warp::redirect::temporary(uri))
                }
            });
    let user_auth = auth_manager.clone();
    let user = warp::get()
        .and(warp::path!("account" / String))
        .and(warp::query::<TokenQuery>())
        .and_then(move |id: String, token: TokenQuery| {
            let tmp_auth = user_auth.clone();
            async move {
                if Auth::is_authenticated(tmp_auth, id.clone(), token.token).await {
                    if let Ok(user) = Account::load(&id).await {
                        Ok::<_, warp::Rejection>(warp::reply::json(&user))
                    } else {
                        Err(warp::reject::not_found())
                    }
                } else {
                    Err(warp::reject::reject())
                }
            }
        });
    let guild_send_auth = auth_manager.clone();
    let guild_send = warp::get()
        .and(warp::path!("guilds/send_message" / ID / ID))
        .and(warp::query::<UserQuery>())
        .and(warp::body::json::<MessageBody>())
        .and_then(move |guild: ID, channel: ID, query: UserQuery, message: MessageBody| {
            let tmp_auth = guild_send_auth.clone();
            async move {
                (if Auth::is_authenticated(tmp_auth, query.account.clone(), query.token).await {
                    if let Ok(account) = Account::load(&query.account).await {
                        if let Err(err) = account.send_guild_message(query.user, guild, channel, message.content).await {
                            Ok(match err {
                                SendMessageError::GuildNotFound | SendMessageError::NotInGuild => warp::reply::with_status("You are not in that guild if it exists.", StatusCode::NOT_FOUND),
                                SendMessageError::ChannelNotFound | SendMessageError::NoPermission => warp::reply::with_status("You do not have permission to access that channel if it exists.", StatusCode::NOT_FOUND),
                                SendMessageError::OpenFileError | SendMessageError::WriteFileError => warp::reply::with_status("Server could not save your message.", StatusCode::INTERNAL_SERVER_ERROR),
                                SendMessageError::UserNotFound => warp::reply::with_status("That user does not exist on your account.", StatusCode::INTERNAL_SERVER_ERROR),
                            })
                        } else {
                            Ok(warp::reply::with_status("Message sent successfully.", StatusCode::OK))
                        }
                    } else {
                        Ok(warp::reply::with_status("That account no longer exists.", StatusCode::NOT_FOUND))
                    }
                } else {
                    Ok(warp::reply::with_status("Invalid authentication details.", StatusCode::FORBIDDEN))
                }) as Result<WithStatus<&str>, Rejection>
            }
        });
    let guilds = warp::path!("guilds").and(guild_send);
    warp::path("v1")
        .and(login.or(user).or(auth))
        .or(guilds)
        .boxed()
}
