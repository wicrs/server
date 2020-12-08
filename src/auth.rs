use std::{collections::HashMap, str::FromStr, sync::Arc};

use base64::URL_SAFE_NO_PAD;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{get_system_millis, user::Account, USER_AGENT_STRING};

use oauth2::{basic::BasicClient, reqwest::http_client, AuthorizationCode};
use oauth2::{AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenResponse, TokenUrl};

type SessionMap = Arc<Mutex<HashMap<String, String>>>;
type LoginSession = (u128, BasicClient);
type LoginSessionMap = Arc<Mutex<HashMap<String, LoginSession>>>;

#[derive(Deserialize, Serialize)]
pub struct AuthQuery {
    state: String,
    code: String,
}

#[derive(Deserialize, Serialize)]
pub struct AccessToken {
    access_token: String,
}

pub enum Service {
    GitHub,
}

impl FromStr for Service {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "github" => Ok(Self::GitHub),
            _ => Err(()),
        }
    }
}

pub struct Auth {
    github: Arc<Mutex<GitHub>>,
    sessions: SessionMap,
}

impl Auth {
    pub fn from_config() -> Self {
        std::fs::create_dir_all("data/accounts")
            .expect("Failed to create the ./data/accounts directory.");
        let auth_config = crate::config::load_config().auth_services;
        let github_conf = auth_config.github.expect(
            "GitHub is currently the only support authentication provider, it cannot be empty.",
        );
        Self {
            github: Arc::new(Mutex::new(GitHub::new(
                github_conf.client_id,
                github_conf.client_secret,
            ))),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn is_authenticated(
        manager: Arc<Mutex<Self>>,
        id: String,
        token: AccessToken,
    ) -> bool {
        let sessions_arc;
        let sessions_lock;
        {
            let lock = manager.lock().await;
            sessions_arc = lock.sessions.clone();
            sessions_lock = sessions_arc.lock().await;
        }
        if let Some(auth_token) = sessions_lock.get(&id) {
            token.access_token == auth_token.clone()
        } else {
            false
        }
    }

    pub async fn start_login(manager: Arc<Mutex<Self>>, service: Service) -> String {
        match service {
            Service::GitHub => {
                let service_arc;
                let service_lock;
                {
                    let lock = manager.lock().await;
                    service_arc = lock.github.clone();
                    service_lock = service_arc.lock().await;
                }
                service_lock.start_login().await
            }
        }
    }

    pub async fn handle_oauth(
        manager: Arc<Mutex<Self>>,
        service: Service,
        query: AuthQuery,
    ) -> (String, Option<(String, String)>) {
        match service {
            Service::GitHub => {
                let service_arc;
                let service_lock;
                {
                    let lock = manager.lock().await;
                    service_arc = lock.github.clone();
                    service_lock = service_arc.lock().await;
                }
                service_lock
                    .handle_oauth(manager, query.state, query.code)
                    .await
            }
        }
    }

    async fn finalize_login(
        manager: Arc<Mutex<Self>>,
        service: &str,
        id: &str,
        email: String,
    ) -> (bool, Option<(String, String)>) {
        let account_existed;
        let account;
        if let Ok(loaded_account) = Account::load_get_id(id, service).await {
            account = loaded_account;
            account_existed = true;
        } else {
            account_existed = false;
            let new_account = Account::new(id.to_string(), email, service.to_string());
            if let Ok(_) = new_account.save().await {
                account = new_account;
            } else {
                return (account_existed, None);
            }
        }
        let id = account.id.to_string();
        let mut vec: Vec<u8> = Vec::with_capacity(64);
        for _ in 0..vec.capacity() {
            vec.push(rand::random());
        }
        let token = base64::encode_config(vec, URL_SAFE_NO_PAD);
        {
            let sessions_arc;
            let mut sessions_lock;
            {
                let lock = manager.lock().await;
                sessions_arc = lock.sessions.clone();
                sessions_lock = sessions_arc.lock().await;
            }
            sessions_lock.insert(id.clone(), token.clone());
        }
        (account_existed, Some((id, token)))
    }
}

enum AuthGetError {
    NoResponse,
    BadJson,
}

struct GitHub {
    client: reqwest::Client,
    client_id: ClientId,
    client_secret: ClientSecret,
    auth_url: AuthUrl,
    token_url: TokenUrl,
    sessions: LoginSessionMap,
}

impl GitHub {
    fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            client_id: ClientId::new(client_id),
            client_secret: ClientSecret::new(client_secret),
            auth_url: AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
                .expect("Invalid GitHub authorization endpoint URL"),
            token_url: TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
                .expect("Invalid GitHub token endpoint URL"),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_id(&self, token: &String) -> Result<String, AuthGetError> {
        let user_request = self
            .client
            .get("https://api.github.com/user")
            .header(USER_AGENT, USER_AGENT_STRING)
            .header(AUTHORIZATION, "token ".to_owned() + token)
            .send()
            .await;
        if let Ok(response) = user_request {
            if let Ok(json) = response.json::<Value>().await {
                Ok(json["id"].to_string())
            } else {
                Err(AuthGetError::BadJson)
            }
        } else {
            Err(AuthGetError::NoResponse)
        }
    }

    async fn get_email(&self, token: &String) -> Result<String, AuthGetError> {
        let email_request = self
            .client
            .get("https://api.github.com/user/emails")
            .header(USER_AGENT, USER_AGENT_STRING)
            .header(AUTHORIZATION, "token ".to_owned() + token)
            .send()
            .await;
        if let Ok(response) = email_request {
            if let Ok(json) = response.json::<Value>().await {
                if let Some(email_array) = json.as_array() {
                    for email_entry in email_array {
                        if let Some(is_primary) = email_entry["primary"].as_bool() {
                            if is_primary {
                                return Ok(email_entry["email"].to_string());
                            }
                        }
                    }
                }
                Err(AuthGetError::BadJson)
            } else {
                Err(AuthGetError::BadJson)
            }
        } else {
            Err(AuthGetError::NoResponse)
        }
    }

    async fn get_session(&self, state: &String) -> Option<LoginSession> {
        let arc = self.sessions.clone();
        let mut lock = arc.lock().await;
        lock.remove(state)
    }

    async fn start_login(&self) -> String {
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
        {
            let arc = self.sessions.clone();
            let mut lock = arc.lock().await;
            lock.insert(csrf_state.secret().clone(), (get_system_millis(), client));
        }
        authorize_url.to_string()
    }

    async fn handle_oauth(
        &self,
        manager: Arc<Mutex<Auth>>,
        state: String,
        code: String,
    ) -> (String, Option<(String, String)>) {
        if let Some(client) = self.get_session(&state).await {
            let code = AuthorizationCode::new(code.clone());
            if let Ok(token) = client.1.exchange_code(code).request(http_client) {
                let token = token.access_token().secret();
                if let Ok(id) = self.get_id(&token).await {
                    if let Ok(email) = self.get_email(&token).await {
                        let auth = Auth::finalize_login(manager, "github", &id, email).await;
                        if let Some(info) = auth.1 {
                            if auth.0 {
                                return (
                                    String::from(format!(
                                    "Signed in using GitHub to ID {}.\nYour access token is: {}",
                                    info.0, info.1
                                )),
                                    Some((info.0, info.1)),
                                );
                            } else {
                                return (String::from(format!("Signed up using GitHub, your ID is {}.\nYour access token is: {}", info.0, info.1)), Some((info.0, info.1)));
                            }
                        }
                        if auth.0 {
                            return (String::from("Sign in failed."), None);
                        } else {
                            return (String::from("Sign up failed."), None);
                        }
                    }
                    return (
                        String::from("Failed to get the primary email of your GitHub account."),
                        None,
                    );
                }
                return (
                    String::from("Failed to get the ID of your GitHub account."),
                    None,
                );
            }
            return (
                String::from("Failed to get an access token from the code."),
                None,
            );
        }
        return (String::from("Invalid session."), None);
    }
}
