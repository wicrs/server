use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::{
    account_not_found_response, auth::Auth, get_system_millis, hub::Hub, is_valid_username, new_id,
    unexpected_response, ApiActionError, JsonLoadError, JsonSaveError, ID, NAME_ALLOWED_CHARS,
};

static ACCOUNT_FOLDER: &str = "data/users/";

/// An Account, a "subidenty" for a user, exists so that a user can have multiple accounts without signing up multiple times, if
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub parent_id: String,
    pub is_bot: bool,
    pub in_hubs: Vec<ID>,
}

/// A version of an Account, uses a hashed version of the list of hubs the user is a member of, the is_bot variable indicates whether or not the account is dedicated to automation/chatbot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericAccount {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub parent_id: String,
    pub is_bot: bool,
    pub hubs_hashed: Vec<String>,
}

impl Account {
    /// Create a new account, while checking that the username only contains allowed characters, if it doesnt an error is returned.
    pub fn new(id: ID, username: String, parent_id: String, is_bot: bool) -> Result<Self, ()> {
        if is_valid_username(&username) {
            Ok(Self {
                id,
                username,
                parent_id,
                created: get_system_millis(),
                is_bot,
                in_hubs: Vec::new(),
            })
        } else {
            Err(())
        }
    }

    /// Hashes the IDs of hubs the account is a member of and outputs a GenericAccount
    pub fn to_generic(&self) -> GenericAccount {
        let mut hasher = Sha256::new();
        let mut hubs_hashed = Vec::new();
        for hub in self.in_hubs.clone() {
            hasher.update(hub.to_string());
            hubs_hashed.push(format!("{:x}", hasher.finalize_reset()));
        }
        GenericAccount {
            id: self.id.clone(),
            created: self.created.clone(),
            username: self.username.clone(),
            parent_id: self.parent_id.clone(),
            is_bot: self.is_bot.clone(),
            hubs_hashed,
        }
    }
}

/// Represents a user, keeps track of which accounts it owns and their metadata.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created: u128,
    pub service: String,
    pub accounts: HashMap<ID, Account>,
}

/// Represents the publicly available information on a user, (excludes their email address and the service they signed up with) also only includes the generic version of accounts.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericUser {
    id: String,
    created: u128,
    users: HashMap<ID, GenericAccount>,
}

impl User {
    /// Creates a new user and generates an ID by hashing the service used and the ID of the user according to that service.
    pub fn new(id: String, email: String, service: String) -> Self {
        Self {
            id: get_id(&id, &service),
            email,
            service,
            accounts: HashMap::new(),
            created: get_system_millis(),
        }
    }

    /// Converts a HashMap of Accounts into a a HashMap of Generic Accounts.
    pub fn accounts_generic(users: &HashMap<ID, Account>) -> HashMap<ID, GenericAccount> {
        users
            .iter()
            .map(|e| (e.0.clone(), e.1.to_generic()))
            .collect()
    }

    /// Converts the standard user into a GenericUser.
    pub fn to_generic(&self) -> GenericUser {
        GenericUser {
            id: self.id.clone(),
            created: self.created.clone(),
            users: Self::accounts_generic(&self.accounts),
        }
    }

    pub async fn create_new_account(
        &mut self,
        username: String,
        bot: bool,
    ) -> Result<Account, ApiActionError> {
        let uuid = new_id();
        if let Ok(user) = Account::new(uuid.clone(), username, self.id.clone(), bot) {
            self.accounts.insert(uuid, user.clone());
            if let Ok(_) = self.save().await {
                Ok(user)
            } else {
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::BadNameCharacters)
        }
    }

    pub async fn send_hub_message(
        &self,
        account: ID,
        hub: ID,
        channel: ID,
        message: String,
    ) -> Result<ID, ApiActionError> {
        if let Some(account) = self.accounts.get(&account) {
            if account.in_hubs.contains(&hub) {
                if let Ok(mut hub) = Hub::load(&hub.to_string()).await {
                    hub.send_message(account.id, channel, message).await
                } else {
                    Err(ApiActionError::HubNotFound)
                }
            } else {
                Err(ApiActionError::NotInHub)
            }
        } else {
            Err(ApiActionError::UserNotFound)
        }
    }

    pub async fn create_hub(
        &mut self,
        name: String,
        id: ID,
        owner: ID,
    ) -> Result<ID, ApiActionError> {
        if !name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
            return Err(ApiActionError::BadNameCharacters);
        }
        if let Some(account) = self.accounts.get_mut(&owner) {
            let new_hub = Hub::new(name, id, account);
            if let Ok(_) = new_hub.save().await {
                account.in_hubs.push(new_hub.id.clone());
                if let Ok(_) = self.save().await {
                    Ok(new_hub.id)
                } else {
                    Err(ApiActionError::WriteFileError)
                }
            } else {
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::UserNotFound)
        }
    }

    pub async fn save(&self) -> Result<(), JsonSaveError> {
        if let Err(_) = tokio::fs::create_dir_all(ACCOUNT_FOLDER).await {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = std::fs::write(
                ACCOUNT_FOLDER.to_owned() + &self.id.to_string() + ".json",
                json,
            ) {
                Ok(result)
            } else {
                Err(JsonSaveError::WriteFile)
            }
        } else {
            Err(JsonSaveError::Serialize)
        }
    }

    pub async fn load(id: &str) -> Result<Self, JsonLoadError> {
        if let Ok(json) = tokio::fs::read_to_string(ACCOUNT_FOLDER.to_owned() + id + ".json").await
        {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(JsonLoadError::Deserialize)
            }
        } else {
            Err(JsonLoadError::ReadFile)
        }
    }

    pub async fn load_get_id(id: &str, service: &str) -> Result<Self, JsonLoadError> {
        Self::load(&get_id(id, service)).await
    }
}

pub fn get_id(id: &str, service: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id);
    hasher.update(service);
    format!("{:x}", hasher.finalize())
}

api_get! { (api_v1_userinfo,,) [auth, user, query]
        warp::reply::json(&user).into_response()
}

fn api_v1_userinfo_noauth() -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path!(String))
        .and_then(|id: String| async move {
            Ok::<_, warp::Rejection>(if let Ok(user) = User::load(&id).await {
                warp::reply::json(&user.to_generic()).into_response()
            } else {
                account_not_found_response()
            })
        })
        .boxed()
}

#[derive(Deserialize)]
struct CreateAccount {
    name: String,
    is_bot: bool
}

api_get! { (api_v1_addaccount, CreateAccount,) [auth, user, query]
        use crate::ApiActionError;
        let mut user = user;
        let create: Result<Account, ApiActionError> = user.create_new_account(query.name, query.is_bot).await;
        if let Ok(account) = create {
            warp::reply::json(&account).into_response()
        } else {
            match create.err() {
                Some(ApiActionError::WriteFileError) => warp::reply::with_status(
                    "Server could not write user data to disk.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response(),
                Some(ApiActionError::BadNameCharacters) => warp::reply::with_status(
                    format!(
                        "Username string can only contain the following characters: \"{}\"",
                        NAME_ALLOWED_CHARS
                    ),
                    StatusCode::BAD_REQUEST,
                )
                .into_response(),
                _ => unexpected_response(),
            }
        }
}

pub fn api_v1(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    warp::path("user")
        .and(
            (warp::path("addaccount").and(api_v1_addaccount(auth_manager.clone())))
                .or(api_v1_userinfo(auth_manager.clone()))
                .or(api_v1_userinfo_noauth()),
        )
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::hub::Hub;

    use super::{get_id, GenericUser, User, ID};
    use super::{Account, GenericAccount};

    static USER_ID: &str = "b5aefca491710ba9965c2ef91384210fbf80d2ada056d3229c09912d343ac6b0";
    static SERVICE_USER_ID: &str = "testid";
    static EMAIL: &str = "test@example.com";

    #[test]
    fn id_gen() {
        assert_eq!(get_id(SERVICE_USER_ID, "github"), USER_ID.to_string());
        assert_ne!(get_id(SERVICE_USER_ID, "other"), USER_ID.to_string());
    }

    #[test]
    fn new_account() {
        let user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        assert_eq!(user.id, USER_ID.to_string());
    }

    #[tokio::test]
    #[serial]
    async fn create_new_user() {
        let mut user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        assert!(user.accounts.is_empty());
        let account = user
            .create_new_account("user".to_string(), false)
            .await
            .expect("Failed to add a new user to the test account.");
        assert_eq!(user.accounts.get(&account.id), Some(&account));
    }

    fn account_generic_pair(n: u128) -> (ID, Account, GenericAccount) {
        let id = ID::from_u128(n);
        let account = Account {
            id: id.clone(),
            username: "test".to_string(),
            created: n,
            is_bot: false,
            in_hubs: Vec::new(),
            parent_id: USER_ID.to_string(),
        };
        let generic = GenericAccount {
            id: id.clone(),
            username: "test".to_string(),
            created: n,
            is_bot: false,
            hubs_hashed: Vec::new(),
            parent_id: USER_ID.to_string(),
        };
        (id, account, generic)
    }

    #[test]
    fn account_to_generic() {
        let uuid = ID::from_u128(0);
        let account = Account::new(
            uuid.clone(),
            "Test_with-chars. And".to_string(),
            USER_ID.to_string(),
            false
        )
        .expect("Valid username was marked as invalid.");
        let generic = GenericAccount {
            id: uuid,
            username: "Test_with-chars. And".to_string(),
            created: account.created,
            is_bot: false,
            parent_id: USER_ID.to_string(),
            hubs_hashed: Vec::new(),
        };
        assert_eq!(account.to_generic(), generic);
    }

    #[tokio::test]
    #[serial]
    async fn user_to_generic() {
        let mut user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        let account_0 = user
            .create_new_account("user".to_string(), false)
            .await
            .expect("Failed to add a new user to the test account.");
        let account_1 = user
            .create_new_account("user".to_string(), false)
            .await
            .expect("Failed to add a new user to the test account.");
        let account_2 = user
            .create_new_account("user".to_string(), false)
            .await
            .expect("Failed to add a new user to the test account.");
        let mut map: HashMap<ID, GenericAccount> = HashMap::new();
        map.insert(account_0.id.clone(), account_0.to_generic().clone());
        map.insert(account_1.id.clone(), account_1.to_generic().clone());
        map.insert(account_2.id.clone(), account_2.to_generic().clone());
        let generic = GenericUser {
            id: USER_ID.to_string(),
            created: user.created,
            users: map,
        };
        assert_eq!(user.to_generic(), generic);
    }

    #[test]
    fn vec_to_generic() {
        let mut map_account: HashMap<ID, Account> = HashMap::new();
        let mut map_generic: HashMap<ID, GenericAccount> = HashMap::new();
        let set_0 = account_generic_pair(0);
        let set_1 = account_generic_pair(1);
        let set_2 = account_generic_pair(2);
        map_account.insert(set_0.0, set_0.1);
        map_account.insert(set_1.0, set_1.1);
        map_account.insert(set_2.0, set_2.1);
        map_generic.insert(set_0.0, set_0.2);
        map_generic.insert(set_1.0, set_1.2);
        map_generic.insert(set_2.0, set_2.2);
        assert_eq!(User::accounts_generic(&map_account), map_generic);
    }

    #[tokio::test]
    #[serial]
    async fn user_save_load() {
        let user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "service".to_string(),
        );
        let _delete = std::fs::remove_file("data/users/".to_string() + &user.id);
        let _save = user.save().await;
        let loaded = User::load(&user.id)
            .await
            .expect("Failed to load the test account from disk.");
        assert_eq!(user, loaded);
    }

    #[tokio::test]
    #[serial]
    async fn create_hub() {
        let mut user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        let _delete = std::fs::remove_file("data/users/".to_string() + &user.id);
        let account = user
            .create_new_account("test".to_string(), false)
            .await
            .expect("Failed to create account for test user.");
        let id = ID::from_u128(0);
        let _delete = std::fs::remove_file("data/hubs/info/".to_string() + &id.to_string());
        let hub = user
            .create_hub("test_hub".to_string(), id.clone(), account.id)
            .await
            .expect("Failed to create test hub.");
        assert!(std::path::Path::new(
            &("data/hubs/info/".to_string() + &hub.to_string() + ".json")
        )
        .exists());
    }

    #[tokio::test]
    #[serial]
    async fn send_hub_message() {
        let mut account = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        let user = account
            .create_new_account("test".to_string(), false)
            .await
            .expect("Failed to create account for test user.");
        let id = ID::from_u128(0);
        let hub_id = account
            .create_hub("test_hub".to_string(), id.clone(), user.id)
            .await
            .expect("Failed to create test hub.");
        let mut hub = Hub::load(&hub_id.to_string())
            .await
            .expect("Failed to load test hub.");
        let channel = hub
            .new_channel(user.id, "test_channel".to_string())
            .await
            .expect("Failed to create test channel.");
        hub.save().await.expect("Failed to save test hub.");
        account
            .send_hub_message(user.id, hub_id, channel.clone(), "test".to_string())
            .await
            .expect("Failed to send message.");
        let channel = hub
            .channels
            .get(&channel)
            .expect("Failed to load test channel.");
        assert!(!channel
            .find_messages_containing("test".to_string(), true)
            .await
            .is_empty());
    }
}
