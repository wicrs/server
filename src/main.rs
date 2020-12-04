use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

use user::{Account, User};
use uuid::Uuid;

use tokio::sync::Mutex;

use warp::{
    http::{Response, StatusCode, Uri},
    Filter,
};

use oauth2::{
    basic::BasicClient, basic::BasicErrorResponseType, basic::BasicTokenType, reqwest::http_client,
    AuthorizationCode, Client, EmptyExtraTokenFields, StandardErrorResponse, StandardTokenResponse,
};
use oauth2::{AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenResponse, TokenUrl};

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, USER_AGENT};

use serde::{Deserialize, Serialize};

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

static USER_AGENT_STRING: &str = "wirc_server";

#[derive(Deserialize, Serialize)]
struct AuthQuery {
    state: String,
    code: String,
}

#[derive(Deserialize, Serialize)]
struct TokenQuery {
    access_token: String,
}

#[tokio::main]
async fn main() {
    data_prep().await;
    let logged_in = Arc::new(Mutex::new(HashMap::<u128, (String, Account)>::new()));
    let authenticator = Arc::new(Mutex::new(Authenticator::github()));
    let login_auth = authenticator.clone();
    let response_auth = authenticator.clone();
    let auth_users = logged_in.clone();
    let authenticate = warp::get()
        .and(warp::query::<AuthQuery>())
        .map(move |auth: AuthQuery| {
            let entry = futures::executor::block_on(async {
                let result;
                {
                    let arc = response_auth.clone();
                    let mut lock = arc.lock().await;
                    result = lock.in_progress.remove(&auth.state);
                }
                result.clone()
            });
            let response = match entry {
                Some(client) => futures::executor::block_on(handle_oauth(
                    auth_users.clone(),
                    client.1,
                    auth.code,
                )),
                None => "Invalid login session.".to_string(),
            };
            Response::builder().status(StatusCode::OK).body(response)
        });
    let login = warp::get().and(warp::path("login")).map(move || {
        let url;
        {
            let result = futures::executor::block_on(async {
                let redirect;
                {
                    let arc = login_auth.clone();
                    let mut lock = arc.lock().await;
                    redirect = lock.new_redirect()
                }
                redirect.clone().parse::<Uri>().unwrap()
            });
            url = result.clone();
            std::mem::drop(result);
        }
        warp::redirect::temporary(url)
    });
    let query_users = logged_in.clone();
    let user = warp::get()
        .and(warp::path!("user" / u128))
        .and(warp::query::<TokenQuery>())
        .map(move |id: u128, token: TokenQuery| {
            let query_users_arc = query_users.clone();
            let user = futures::executor::block_on(async move {
                let arc = query_users_arc.clone();
                let lock = arc.lock().await;
                if let Some(account) = lock.get(&id).clone() {
                    Some((account.0.clone(), account.1.clone()))
                } else {
                    None
                }
            });
            if let Some(user) = user {
                if user.0 == token.access_token {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, "application/json")
                        .body(serde_json::to_string_pretty(&user.1).unwrap())
                } else {
                    Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .body("Bad access token.".to_string())
                }
            } else {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body("Could not find that user.".to_string())
            }
        });
    warp::serve(login.or(user).or(authenticate))
        .run(([127, 0, 0, 1], 24816))
        .await;
}

struct Authenticator {
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    in_progress: HashMap<String, (u128, BasicClient)>, // state (time created, client)
}

impl Authenticator {
    pub fn github() -> Self {
        let config_json =
            std::fs::read_to_string("config.json").expect("Could not read config file.");
        let config = serde_json::from_str::<Value>(&config_json)
            .expect("Config file contains invalid JSON.");
        let client_id = ClientId::new(
            config["github_client_id"]
                .as_str()
                .expect("Invalid GitHub client ID in config.")
                .to_string(),
        );
        let client_secret = ClientSecret::new(
            config["github_client_secret"]
                .as_str()
                .expect("Invalid GitHub client secret in config.")
                .to_string(),
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
            in_progress: HashMap::new(),
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
        self.in_progress
            .insert(csrf_state.secret().clone(), (get_system_millis(), client));
        authorize_url.to_string()
    }
}

pub async fn handle_oauth(
    logged_in: Arc<Mutex<HashMap<u128, (String, Account)>>>,
    client: Client<
        StandardErrorResponse<BasicErrorResponseType>,
        StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        BasicTokenType,
    >,
    code: String,
) -> String {
    let code = AuthorizationCode::new(code.clone());
    if let Ok(token) = client.exchange_code(code).request(http_client) {
        let token = token.access_token().secret();
        let token_header = "token ".to_owned() + token;
        let client = reqwest::blocking::Client::new();
        let user_request = client
            .get("https://api.github.com/user")
            .header(USER_AGENT, USER_AGENT_STRING)
            .header(AUTHORIZATION, token_header.clone())
            .send();
        if let Ok(response) = user_request {
            if let Ok(user_json) = response.json::<Value>() {
                let id = user_json["id"].to_string();
                let id_num: u128 = id.parse().unwrap();
                let name = user_json["login"].to_string();
                if let Ok(account) = Account::load(id.clone(), "github".to_string()) {
                    return format!(
                        "Signed in using GitHub account {}.\nYour access token is: {}.",
                        name,
                        login(logged_in, account, token.clone()).await
                    );
                } else {
                    let email_request = client
                        .get("https://api.github.com/user/emails")
                        .header(USER_AGENT, USER_AGENT_STRING)
                        .header(AUTHORIZATION, token_header)
                        .send();
                    if let Ok(response) = email_request {
                        if let Ok(email_json) = response.json::<Value>() {
                            if let Some(email_array) = email_json.as_array() {
                                for email_entry in email_array {
                                    if email_entry["primary"].as_bool().unwrap() {
                                        let mut new_account = Account::new(
                                            id_num,
                                            email_entry["email"].to_string(),
                                            "github".to_string(),
                                        );
                                        new_account.users.push(User::new(
                                            name.clone(),
                                            false,
                                            new_account.id.clone(),
                                        ));
                                        if let Ok(_) = new_account.save() {
                                            let email = new_account.email.clone();
                                            return format!("Signed up using GitHub account {} with ID {}, email is set to {}.\nYour access token is: {}.", name, id, email, login(logged_in, new_account, token.clone()).await);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    "Failed to authenticate with GitHub.".to_string()
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

pub async fn login(
    logged_in: Arc<Mutex<HashMap<u128, (String, Account)>>>,
    account: Account,
    oauth_token: String,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(oauth_token);
    hasher.update(account.id.to_string());
    hasher.update(account.email.clone());
    let hash = format!("{:X}", hasher.finalize());
    {
        logged_in
            .clone()
            .lock()
            .await
            .insert(account.id.clone(), (hash.clone(), account));
    }
    return hash;
}
