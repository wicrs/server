use std::sync::Arc;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};
use serde_json::Value;

use uuid::Uuid;

use tokio::sync::Mutex;

use warp::{
    http::{Response, StatusCode, Uri},
    Filter,
};

use oauth2::{AuthorizationCode, basic::BasicTokenType, EmptyExtraTokenFields, StandardTokenResponse, basic::BasicClient, reqwest::http_client};
use oauth2::{AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenUrl};

pub mod channel;
pub mod guild;
pub mod message;
pub mod permission;
pub mod user;

pub enum JsonLoadError {
    ReadFile,
    Deserialize,
}

pub enum JsonSaveError {
    WriteFile,
    Serialize,
    Directory,
}

#[tokio::main]
async fn main() {
    data_prep().await;
    let authenticator = Arc::new(Mutex::new(Authenticator::github()));
    let login_auth = authenticator.clone();
    let response_auth = authenticator.clone();
    let authenticate = warp::get()
        .and(warp::query::<HashMap<String, String>>())
        .map(
            move |query: HashMap<String, String>| match query.get("code") {
                Some(code) => match query.get("state") {
                    Some(state) => {
                        response_auth
                            .clone()
                            .try_lock()
                            .unwrap()
                            .response(state.to_owned(), code.to_owned());
                        Response::builder().status(StatusCode::OK).body("")
                    }
                    None => Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body("Missing state parameter."),
                },
                None => Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Missing code parameter."),
            },
        );
    let login = warp::get().and(warp::path("login")).map(move || {
        warp::redirect::temporary(
            login_auth
                .clone()
                .try_lock()
                .unwrap()
                .new_redirect()
                .parse::<Uri>()
                .unwrap(),
        )
    });
    warp::serve(login.or(authenticate))
        .run(([127, 0, 0, 1], 24816))
        .await;
}

struct Session {
    client: BasicClient,
    token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>
}

struct Authenticator {
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    in_progress: HashMap<String, (u128, BasicClient)>,
    logged_in: HashMap<String, Session>,
}

impl Authenticator {
    pub fn github() -> Self {
        let config_json = std::fs::read_to_string("config.json").expect("Could not read config file.");
        let config = serde_json::from_str::<Value>(&config_json).expect("Config file contains invalid JSON.");
        let client_id = ClientId::new(config["github_client_id"].as_str().expect("Invalid GitHub client ID in config.").to_string());
        let client_secret = ClientSecret::new(config["github_client_secret"].as_str().expect("Invalid GitHub client secret in config.").to_string());
        let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
            .expect("Invalid authorization endpoint URL");
        let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
            .expect("Invalid token endpoint URL");
        Self {
            client_id,
            client_secret,
            auth_url,
            token_url,
            in_progress: HashMap::new(),
            logged_in: HashMap::new(),
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
        self.in_progress.insert(csrf_state.secret().clone(), (get_system_millis(), client));
        authorize_url.to_string()
    }

    pub fn response(&mut self, state: String, code: String) {
        match self.in_progress.get_key_value(&state) {
            Some(client) => {
                let code = AuthorizationCode::new(code);
                if let Ok(token) = client.1.1.exchange_code(code).request(http_client) {
                    self.logged_in.insert("".to_string(), Session { token, client: self.in_progress.remove(&state).unwrap().1 });
                }
            }
            None => {}
        }
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

pub async fn data_prep() {
    std::fs::create_dir_all("data/accounts")
        .expect("Failed to create the ./data/accounts directory.");
}
