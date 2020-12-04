use serde_json::Value;
use std::sync::Arc;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use user::Account;
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
    let logged_in = Arc::new(Mutex::new(HashMap::<String, Account>::new()));
    let authenticator = Arc::new(Mutex::new(Authenticator::github()));
    let login_auth = authenticator.clone();
    let response_auth = authenticator.clone();
    let authenticate = warp::get()
        .and(warp::query::<HashMap<String, String>>())
        .map(
            move |query: HashMap<String, String>| match query.get("code") {
                Some(code) => match query.get("state") {
                    Some(state) => {
                        let entry = futures::executor::block_on(async {
                            let result;
                            {
                                let arc = response_auth.clone();
                                let mut lock = arc.lock().await;
                                result = lock.in_progress.remove(state);
                            }
                            result.clone()
                        });
                        let response = match entry {
                            Some(client) => futures::executor::block_on(handle_oauth(
                                logged_in.clone(),
                                client.1,
                                code.clone(),
                            )),
                            None => "Invalid session state ID.".to_string(),
                        };
                        Response::builder().status(StatusCode::OK).body(response)
                    }
                    None => Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body("Missing state parameter.".to_string()),
                },
                None => Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body("Missing code parameter.".to_string()),
            },
        );
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
    warp::serve(login.or(authenticate))
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
    logged_in: Arc<Mutex<HashMap<String, Account>>>,
    client: Client<
        StandardErrorResponse<BasicErrorResponseType>,
        StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
        BasicTokenType,
    >,
    code: String,
) -> String {
    let code = AuthorizationCode::new(code.clone());
    if let Ok(token) = client.exchange_code(code).request(http_client) {
        if let Ok(response) = reqwest::Client::new()
            .get("https://api.github.com/user")
            .header(
                "Authorization",
                "token ".to_owned() + token.access_token().secret(),
            )
            .send()
            .await
        {
            if let Ok(json) = response.json::<Value>().await {
                if let Some(id) = json["id"].as_str() {
                    if let Ok(account) = Account::load(id.to_string(), "github".to_string()) {
                        let email = account.email.clone();
                        {
                            logged_in
                                .clone()
                                .lock()
                                .await
                                .insert(id.to_string(), account);
                        }
                        return "Logged in as ".to_string() + &id + " with email " + &email;
                    } else {
                        if let Ok(response) = reqwest::Client::new()
                            .get("https://api.github.com/user/emails")
                            .header(
                                "Authorization",
                                "token ".to_owned() + token.access_token().secret(),
                            )
                            .send()
                            .await
                        {
                            if let Ok(json) = response.json::<Value>().await {
                                if let Some(array) = json.as_array() {
                                    for e in array {
                                        if e["primary"].as_bool().unwrap() {
                                            let new_account = Account::new(
                                                id.parse().unwrap(),
                                                e["email"].as_str().unwrap().to_string(),
                                                "github".to_string(),
                                            );
                                            if let Ok(_) = new_account.save() {
                                                let email = new_account.email.clone();
                                                {
                                                    logged_in
                                                        .clone()
                                                        .lock()
                                                        .await
                                                        .insert(id.to_string(), new_account);
                                                }
                                                return "Signed up as ".to_string()
                                                    + &id
                                                    + " with email "
                                                    + &email;
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
