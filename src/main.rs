#![feature(async_closure)]
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{
    http::{Response, StatusCode, Uri},
    Filter,
};

use oauth2::basic::BasicClient;

use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenUrl,
};

use std::env;

pub mod channel;
pub mod guild;
pub mod message;
pub mod permission;
pub mod user;

#[tokio::main]
async fn main() {
    let authenticator = Arc::new(Mutex::new(Authenticator::github_from_env()));
    let login_auth = authenticator.clone();
    let response_auth = authenticator.clone();
    let authenticate = warp::get()
        .and(warp::query::<HashMap<String, String>>())
        .map(move |query: HashMap<String, String>| {
            match query.get("code") {
                Some(code) => match query.get("state") {
                    Some(state) => {
                        response_auth
                            .clone().try_lock().unwrap()
                            .response(state.to_owned(), code.to_owned());
                        Response::builder().status(StatusCode::OK).body("")
                    }
                    None => Response::builder().status(StatusCode::BAD_REQUEST).body("Missing parameters."),
                },
                None => Response::builder().status(StatusCode::BAD_REQUEST).body("Missing parameters."),
            }
        });
    let login = warp::get().and(warp::path("login")).map(move || {
        warp::redirect::temporary(login_auth.clone().try_lock().unwrap().new_redirect().parse::<Uri>().unwrap())
    });
    warp::serve(login.or(authenticate)).run(([127, 0, 0, 1], 24816)).await;
}

struct Authenticator {
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    logins: Vec<CsrfToken>
}

impl Authenticator {
    pub fn github_from_env() -> Self {
        let client_id = ClientId::new(
            env::var("GITHUB_CLIENT_ID")
                .expect("Missing the GITHUB_CLIENT_ID environment variable."),
        );
        let client_secret = ClientSecret::new(
            env::var("GITHUB_CLIENT_SECRET")
                .expect("Missing the GITHUB_CLIENT_ID environment variable."),
        );
        let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
            .expect("Invalid token endpoint URL");
        Self {
            client_id,
            client_secret,
            auth_url,
            token_url,
            logins: Vec::new()
        }
    }

    pub fn new_redirect(&mut self) -> String {
        let client = BasicClient::new(
            self.client_id.clone(),
            Some(self.client_secret.clone()),
            self.auth_url.clone(),
            Some(self.token_url.clone()),
        );
        let (authorize_url, csrf_state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("read:user".to_string()))
            .add_scope(Scope::new("user:email".to_string()))
            .url();
        self.logins.push(csrf_state);
        authorize_url.to_string()
    }

    pub fn response(&self, state: String, code: String) {
        println!("state: {}, code: {}", state, code);
    }
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
