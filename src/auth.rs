use std::{collections::HashMap, str::FromStr, sync::Arc};

use base64::URL_SAFE_NO_PAD;
use reqwest::header::{AUTHORIZATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, hyper::Uri, Filter, Rejection, Reply};

use crate::{get_system_millis, user::User, ID, USER_AGENT_STRING};

use oauth2::{basic::BasicClient, reqwest::http_client, AuthorizationCode};
use oauth2::{AuthUrl, ClientId, ClientSecret, CsrfToken, Scope, TokenResponse, TokenUrl};

type SessionMap = Arc<Mutex<HashMap<String, Vec<(u128, String)>>>>;
type LoginSession = (u128, BasicClient);
type LoginSessionMap = Arc<Mutex<HashMap<String, LoginSession>>>;

#[derive(Deserialize, Serialize)]
pub struct AuthQuery {
    state: String,
    code: String,
    expires: Option<u128>,
}

#[derive(Deserialize, Serialize)]
pub struct TokenQuery {
    pub token: String,
}

#[derive(Deserialize, Serialize)]
struct AccountTokenResponse {
    id: String,
    token: String,
}
#[derive(Serialize, Deserialize)]
pub struct UserQuery {
    pub account: String,
    pub user: ID,
    pub token: String,
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
    pub async fn from_config() -> Self {
        std::fs::create_dir_all("data/users")
            .expect("Failed to create the ./data/users directory.");
        let auth_config = crate::config::load_config().auth_services;
        let github_conf = auth_config.github.expect(
            "GitHub is currently the only support authentication provider, it cannot be empty.",
        );
        Self {
            github: Arc::new(Mutex::new(GitHub::new(github_conf.client_id, github_conf.client_secret))),
            sessions: Arc::new(Mutex::new(Auth::load_tokens().await)),
        }
    }

    pub async fn for_testing() -> (Self, String, String) {
        let auth = Self {
            github: Arc::new(Mutex::new(GitHub::new(
                "testing".to_string(),
                "testing".to_string(),
            ))),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        };
        let account = User {
            id: "testuser".to_string(),
            email: "test@example.com".to_string(),
            created: 0,
            service: "testing".to_string(),
            accounts: HashMap::new(),
        };
        account.save().await.expect("Failed to save test account.");
        let token = "testtoken".to_string();
        let hashed = hash_auth(account.id.clone(), token.clone());
        auth.sessions
            .lock()
            .await
            .insert(hashed.0, vec![(u128::MAX, hashed.1)]);
        (auth, account.id, token)
    }

    async fn save_tokens(
        sessions: &HashMap<String, Vec<(u128, String)>>,
    ) -> Result<(), std::io::Error> {
        tokio::fs::write(
            "data/sessions.json",
            serde_json::to_string(sessions).unwrap_or("{}".to_string()),
        )
        .await
    }

    async fn load_tokens() -> HashMap<String, Vec<(u128, String)>> {
        if let Ok(read) = tokio::fs::read_to_string("data/sessions.json").await {
            if let Ok(mut map) = serde_json::from_str::<HashMap<String, Vec<(u128, String)>>>(&read)
            {
                let now = get_system_millis();
                for account in &mut map {
                    account.1.retain(|t| t.0 > now);
                }
                let _save = Auth::save_tokens(&map).await;
                return map;
            }
        }
        return HashMap::new();
    }

    pub async fn is_authenticated(manager: Arc<Mutex<Self>>, id: &str, token_str: String) -> bool {
        let sessions_arc;
        let mut sessions_lock;
        {
            let lock = manager.lock().await;
            sessions_arc = lock.sessions.clone();
            sessions_lock = sessions_arc.lock().await;
        }
        let hashed = hash_auth(id.to_string(), token_str.clone());
        if let Some(auth_tokens) = sessions_lock.get_mut(&hashed.0) {
            let now = get_system_millis();
            auth_tokens.retain(|t| t.0 > now);
            for token in auth_tokens {
                if token.1 == hashed.1 {
                    return true;
                }
            }
            false
        } else {
            false
        }
    }

    pub async fn invalidate_tokens(manager: Arc<Mutex<Self>>, id: &str) {
        let sessions_arc;
        let mut sessions_lock;
        {
            let lock = manager.lock().await;
            sessions_arc = lock.sessions.clone();
            sessions_lock = sessions_arc.lock().await;
        }
        sessions_lock.remove(hash_auth(id.to_string(), String::new()).0.as_str());
        let _save = Auth::save_tokens(&sessions_lock).await;
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
        let expires = query.expires.unwrap_or(get_system_millis() + 604800000);
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
                    .handle_oauth(manager, query.state, query.code, expires)
                    .await
            }
        }
    }

    async fn finalize_login(
        manager: Arc<Mutex<Self>>,
        service: &str,
        id: &str,
        expires: u128,
        email: String,
    ) -> (bool, Option<(String, String)>) {
        let user_existed;
        let user;
        if let Ok(loaded_account) = User::load_get_id(id, service).await {
            user = loaded_account;
            user_existed = true;
        } else {
            user_existed = false;
            let new_account = User::new(id.to_string(), email, service.to_string());
            if let Ok(_) = new_account.save().await {
                user = new_account;
            } else {
                return (user_existed, None);
            }
        }
        let id = user.id.to_string();
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
            let hashed = hash_auth(id.clone(), token.clone());
            if let Some(tokens) = sessions_lock.get_mut(&hashed.0) {
                tokens.push((expires, hashed.1))
            } else {
                sessions_lock.insert(hashed.0, vec![(expires, hashed.1)]);
            }
            let _write = Auth::save_tokens(&sessions_lock).await;
        }
        (user_existed, Some((id, token)))
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
        expires: u128,
    ) -> (String, Option<(String, String)>) {
        if let Some(client) = self.get_session(&state).await {
            let code = AuthorizationCode::new(code.clone());
            if let Ok(token) = client.1.exchange_code(code).request(http_client) {
                let token = token.access_token().secret();
                if let Ok(id) = self.get_id(&token).await {
                    if let Ok(email) = self.get_email(&token).await {
                        let auth =
                            Auth::finalize_login(manager, "github", &id, expires, email).await;
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

fn hash_auth(id: String, token: String) -> (String, String) {
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    let id_hash = format!("{:x}", hasher.finalize_reset());
    hasher.update(token.as_bytes());
    (id_hash, format!("{:x}", hasher.finalize_reset()))
}

fn api_v1_login(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path!("login" / Service))
        .and_then(move |service: Service| {
            let tmp_auth = auth_manager.clone();
            async move {
                let uri_string = Auth::start_login(tmp_auth, service).await;
                let uri = uri_string.parse::<Uri>().unwrap();
                Ok::<_, Rejection>(warp::redirect::temporary(uri).into_response())
            }
        })
        .boxed()
}

fn api_v1_oauth(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path!("auth" / Service))
        .and(warp::query::<AuthQuery>())
        .and_then(move |service: Service, query: AuthQuery| {
            let tmp_auth = auth_manager.clone();
            async move {
                let result = Auth::handle_oauth(tmp_auth, service, query).await;
                Ok::<_, warp::Rejection>(if let Some(id_token) = result.1 {
                    warp::reply::json(&AccountTokenResponse {
                        id: id_token.0,
                        token: id_token.1,
                    })
                    .into_response()
                } else {
                    warp::reply::json(&result.0).into_response()
                })
            }
        })
        .boxed()
}

api_get! { (api_v1_invalidate_tokens,,warp::path("invalidate")) [auth, user, query]
    Auth::invalidate_tokens(auth, &user.id).await;
    warp::reply::with_status("Success!", StatusCode::OK).into_response()
}

pub fn api_v1(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    api_v1_login(auth_manager.clone())
        .or(api_v1_invalidate_tokens(auth_manager.clone()))
        .or(api_v1_oauth(auth_manager.clone()))
        .boxed()
}

#[cfg(test)]
mod tests {
    use crate::get_system_millis;

    use super::HashMap;
    use super::{Arc, Mutex};
    use super::{Auth, GitHub};

    static EMAIL: &str = "test@example.com";
    static SERVICE_USER_ID: &str = "testid";
    static USER_ID: &str = "b5aefca491710ba9965c2ef91384210fbf80d2ada056d3229c09912d343ac6b0";

    pub fn new_auth() -> Arc<Mutex<Auth>> {
        let _delete = std::fs::remove_file("data/users/".to_string() + USER_ID + ".json");
        std::thread::sleep(std::time::Duration::from_millis(50));
        Arc::new(Mutex::new(Auth {
            github: Arc::new(Mutex::new(GitHub::new(
                "testing".to_string(),
                "fakesecret".to_string(),
            ))),
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }))
    }

    #[tokio::test]
    #[serial]
    async fn auth() {
        let auth = new_auth();
        let login = Auth::finalize_login(
            auth.clone(),
            "github",
            SERVICE_USER_ID,
            get_system_millis() + 50,
            EMAIL.to_string(),
        )
        .await;
        assert!(!login.0);
        let token_id = login.1.unwrap();
        assert_eq!(token_id.0.clone(), USER_ID.to_string());
        assert!(token_id.0.clone() == USER_ID.to_string());
        assert!(Auth::is_authenticated(auth.clone(), USER_ID, token_id.1).await);
        let read = std::fs::read_to_string("data/users/".to_string() + USER_ID + ".json").unwrap();
        assert!(read.starts_with(r#"{"id":"b5aefca491710ba9965c2ef91384210fbf80d2ada056d3229c09912d343ac6b0","email":"test@example.com","created":"#) && read.ends_with(r#","service":"github","accounts":{}}"#));
    }

    #[tokio::test]
    #[serial]
    async fn token_expiry() {
        let auth = new_auth();
        let login_0 = Auth::finalize_login(
            auth.clone(),
            "github",
            SERVICE_USER_ID,
            get_system_millis() + 50,
            EMAIL.to_string(),
        )
        .await;
        let login_1 = Auth::finalize_login(
            auth.clone(),
            "github",
            SERVICE_USER_ID,
            get_system_millis() + 100000,
            EMAIL.to_string(),
        )
        .await;
        assert!(!login_0.0.clone() && login_1.0.clone());
        assert!(
            login_0.1.clone().unwrap().0 == login_1.1.clone().unwrap().0
                && login_0.1.clone().unwrap().0 == USER_ID.to_string()
        );
        let token_0 = login_0.1.unwrap().1;
        let token_1 = login_1.1.unwrap().1;
        assert!(Auth::is_authenticated(auth.clone(), USER_ID, token_0.clone()).await);
        assert!(Auth::is_authenticated(auth.clone(), USER_ID, token_1.clone()).await);
        std::thread::sleep(std::time::Duration::from_millis(64));
        assert!(!Auth::is_authenticated(auth.clone(), USER_ID, token_0.clone()).await);
        assert!(Auth::is_authenticated(auth.clone(), USER_ID, token_1.clone()).await);
    }

    #[tokio::test]
    #[serial]
    async fn save_load_sessions() {
        let tokens = vec![
            (get_system_millis() + 10000, "test".to_string()),
            (get_system_millis() + 10000, "test".to_string()),
        ];
        let mut map: HashMap<String, Vec<(u128, String)>> = HashMap::new();
        map.insert(USER_ID.to_string(), tokens.clone());
        assert_eq!(map.get(USER_ID), Some(&tokens));
        let _save = Auth::save_tokens(&map).await;
        let loaded = Auth::load_tokens().await;
        assert_eq!(loaded.get(USER_ID), Some(&tokens));
    }

    #[tokio::test]
    #[serial]
    async fn invalidate_tokens() {
        let auth = new_auth();
        let login = Auth::finalize_login(
            auth.clone(),
            "github",
            SERVICE_USER_ID,
            get_system_millis() + 10000,
            EMAIL.to_string(),
        )
        .await;
        assert!(!login.0.clone());
        assert!(login.1.clone().unwrap().0 == USER_ID.to_string());
        let token = login.1.unwrap().1;
        assert!(Auth::is_authenticated(auth.clone(), USER_ID, token.clone()).await);
        Auth::invalidate_tokens(auth.clone(), USER_ID).await;
        assert!(!Auth::is_authenticated(auth.clone(), USER_ID, token.clone()).await);
    }
}
