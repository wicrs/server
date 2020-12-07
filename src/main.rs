#![feature(async_closure)]

use reqwest::header::HeaderValue;
use std::{sync::Arc, time::{Instant, SystemTime, UNIX_EPOCH}};
use tokio::sync::Mutex;

use auth::{AccessToken, Auth, AuthQuery, Service};
use user::Account;

use uuid::Uuid;

use warp::{http::Uri, Filter, Rejection};

use channel::Message;

pub mod auth;
pub mod channel;
pub mod config;
pub mod guild;
pub mod permission;
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

#[tokio::main]
async fn main() {
    message_test();
    let auth = Arc::new(Mutex::new(Auth::from_config()));
    let max_token_age = config::load_config().token_expiry_time;
    let auth_auth = auth.clone();
    let login_auth = auth.clone();
    let user_auth = auth.clone();
    let authenticate = warp::get()
        .and(warp::path!("authenticate" / Service))
        .and(warp::query::<AuthQuery>())
        .and_then(move |service: Service, query: AuthQuery| {
            let tmp_auth = auth_auth.clone();
            async move {
                let result = Auth::handle_oauth(tmp_auth, service, query).await;
                let mut response = warp::reply::Response::new(result.0.into());
                if let Some(id_token) = result.1 {
                    let id = format!("id={}; SameSite=Strict; Path=/; HttpOnly", id_token.0);
                    let token = format!("token={}; SameSite=Strict; Path=/; Max-Age={}; HttpOnly", id_token.1, max_token_age);
                    let headers = response.headers_mut();
                    headers.append("Set-Cookie", HeaderValue::from_str(&id).unwrap());
                    headers.append("Set-Cookie", HeaderValue::from_str(&token).unwrap());
                }
                Ok::<_, Rejection>(response)
            }
        });
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
    let login_prompt = warp::get()
        .and(warp::path("login"))
        .map(|| warp::reply::html(get_asset("login_select.html")));
    let user = warp::get()
        .and(warp::path!("user" / String))
        .and(warp::query::<AccessToken>())
        .and_then(move |id: String, token: AccessToken| {
            let tmp_auth = user_auth.clone();
            async move {
                if Auth::is_authenticated(tmp_auth, id.clone(), token).await {
                    if let Ok(user) = Account::load(&id) {
                        Ok::<_, warp::Rejection>(warp::reply::json(&user))
                    } else {
                        Err(warp::reject::not_found())
                    }
                } else {
                    Err(warp::reject::reject())
                }
            }
        });
    let assets = warp::path("assets").and(warp::fs::dir("assets"));
    warp::serve(assets.or(login).or(login_prompt).or(user).or(authenticate))
        .run(([127, 0, 0, 1], 24816))
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

fn get_asset(file_name: &str) -> Vec<u8> {
    std::fs::read(&("assets/".to_owned() + file_name)).unwrap_or(Vec::new())
}

fn message_test() {
    let mut messages = Vec::new();
    for i in 0..10000 {
        messages.push(Message {
            id: new_id(),
            sender: new_id(),
            created: get_system_millis(),
            content: "testing, here is a number:\n".to_string() + &i.to_string()
        })
    }
    let mut message_strings = Vec::new();
    let now = Instant::now();
    for message in messages.iter() {
        message_strings.push(message.to_string());
    }
    println!("10000 messages to strings took {} micros.", now.elapsed().as_micros());
    let mut messages_parsed = Vec::new();
    let now = Instant::now();
    for message_string in message_strings.iter() {
        if let Ok(message) = message_string.parse::<Message>() {
            messages_parsed.push(message);
        }
    }
    println!("10000 strings to messages took {} micros.", now.elapsed().as_micros());
    let mut parsed_message_strings = Vec::new();
    let now = Instant::now();
    if let Ok(_) = std::fs::write("message_test", message_strings.join("\n")) {
        println!("Writing 10000 messages to file took {} micros.", now.elapsed().as_micros());
        let now = Instant::now();
        if let Ok(string) = std::fs::read_to_string("message_test") {
            for message_string in string.split('\n').into_iter() {
                parsed_message_strings.push(format!("{:?}", message_string));
            }
        }
        println!("Reading 10000 messages from file took {} micros.", now.elapsed().as_micros());
    }
}
