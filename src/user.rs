use std::{collections::HashMap, sync::Arc};

use crate::{
    account_not_found_response, auth::Auth, get_system_millis, guild::Guild, new_id,
    unexpected_response, ApiActionError, JsonLoadError, JsonSaveError, ID, NAME_ALLOWED_CHARS,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, Filter, Reply};

static ACCOUNT_FOLDER: &str = "data/accounts/";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericUser {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub parent_id: String,
    pub guilds_hashed: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub parent_id: String,
    pub in_guilds: Vec<ID>,
}

impl User {
    pub fn new(id: ID, username: String, parent_id: String) -> Result<Self, ()> {
        if username.len() < 32 && username.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
            Ok(Self {
                id,
                username,
                parent_id,
                created: get_system_millis(),
                in_guilds: Vec::new(),
            })
        } else {
            Err(())
        }
    }

    pub fn to_generic(&self) -> GenericUser {
        let mut hasher = Sha256::new();
        let mut guilds_hashed = Vec::new();
        for guild in self.in_guilds.clone() {
            hasher.update(guild.to_string());
            guilds_hashed.push(format!("{:x}", hasher.finalize_reset()));
        }
        GenericUser {
            id: self.id.clone(),
            created: self.created.clone(),
            username: self.username.clone(),
            parent_id: self.parent_id.clone(),
            guilds_hashed,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericAccount {
    id: String,
    created: u128,
    users: HashMap<ID, GenericUser>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub created: u128,
    pub service: String,
    pub users: HashMap<ID, User>,
}

impl Account {
    pub fn new(id: String, email: String, service: String) -> Self {
        Self {
            id: get_id(&id, &service),
            email,
            service,
            users: HashMap::new(),
            created: get_system_millis(),
        }
    }

    pub fn users_generic(users: &HashMap<ID, User>) -> HashMap<ID, GenericUser> {
        users
            .iter()
            .map(|e| (e.0.clone(), e.1.to_generic()))
            .collect()
    }

    pub fn to_generic(&self) -> GenericAccount {
        GenericAccount {
            id: self.id.clone(),
            created: self.created.clone(),
            users: Self::users_generic(&self.users),
        }
    }

    pub async fn create_new_user(&mut self, username: String) -> Result<User, ApiActionError> {
        let uuid = new_id();
        if let Ok(user) = User::new(uuid.clone(), username, self.id.clone()) {
            self.users.insert(uuid, user.clone());
            if let Ok(_) = self.save().await {
                Ok(user)
            } else {
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::BadNameCharacters)
        }
    }

    pub async fn send_guild_message(
        &self,
        user: ID,
        guild: ID,
        channel: ID,
        message: String,
    ) -> Result<(), ApiActionError> {
        if let Some(user) = self.users.get(&user) {
            if user.in_guilds.contains(&guild) {
                if let Ok(mut guild) = Guild::load(&guild.to_string()).await {
                    guild.send_message(user.id, channel, message).await
                } else {
                    Err(ApiActionError::GuildNotFound)
                }
            } else {
                Err(ApiActionError::NotInGuild)
            }
        } else {
            Err(ApiActionError::UserNotFound)
        }
    }

    pub async fn create_guild(&mut self, name: String, user: ID) -> Result<ID, ApiActionError> {
        if !name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
            return Err(ApiActionError::BadNameCharacters);
        }
        if let Some(user) = self.users.get_mut(&user) {
            let new_guild = Guild::new(name, new_id(), user);
            if let Ok(_) = new_guild.save().await {
                user.in_guilds.push(new_guild.id.clone());
                Ok(new_guild.id)
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

api_get! { (api_v1_accountinfo,,) [auth, account, query]
        warp::reply::json(&account).into_response()
}

fn api_v1_accountinfo_noauth() -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path!(String))
        .and_then(|id: String| async move {
            Ok::<_, warp::Rejection>(if let Ok(account) = Account::load(&id).await {
                warp::reply::json(&account.to_generic()).into_response()
            } else {
                account_not_found_response()
            })
        })
        .boxed()
}

#[derive(Deserialize)]
struct Name {
    name: String,
}

api_get! { (api_v1_adduser, Name,) [auth, account, query]
        use crate::ApiActionError;
        let mut account = account;
        let create: Result<User, ApiActionError> = account.create_new_user(query.name).await;
        if let Ok(user) = create {
            warp::reply::json(&user).into_response()
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
    warp::path("account")
        .and(
            (warp::path("adduser").and(api_v1_adduser(auth_manager.clone())))
                .or(api_v1_accountinfo(auth_manager.clone()))
                .or(api_v1_accountinfo_noauth()),
        )
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{get_id, Account, GenericAccount, ID};

    use super::{GenericUser, User};

    static ACCOUNT_ID: &str = "b5aefca491710ba9965c2ef91384210fbf80d2ada056d3229c09912d343ac6b0";
    static SERVICE_ACCOUNT_ID: &str = "testid";
    static EMAIL: &str = "test@example.com";

    #[test]
    fn id_gen() {
        assert_eq!(get_id(SERVICE_ACCOUNT_ID, "github"), ACCOUNT_ID.to_string());
        assert_ne!(get_id(SERVICE_ACCOUNT_ID, "other"), ACCOUNT_ID.to_string());
    }

    #[test]
    fn username_allowed_check() {
        let uuid = ID::from_u128(0);
        let _good_name = User::new(
            uuid.clone(),
            "Test_with-chars. And".to_string(),
            ACCOUNT_ID.to_string(),
        )
        .expect("Valid username was marked as invalid.");
        let _bad_name_chars = User::new(
            uuid.clone(),
            "Test_with-chars. And!".to_string(),
            ACCOUNT_ID.to_string(),
        )
        .expect_err("Username with illegal characters was marked as valid.");
        let _bad_name_len = User::new(
            uuid,
            "Test_with-chars. And this name is way to long, it should cause an error.".to_string(),
            ACCOUNT_ID.to_string(),
        )
        .expect_err("Username that exceeded maximum length was marked as invalid.");
    }

    #[test]
    fn new_account() {
        let account = Account::new(
            SERVICE_ACCOUNT_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        assert_eq!(account.id, ACCOUNT_ID.to_string());
    }

    #[tokio::test]
    async fn create_new_user() {
        let mut account = Account::new(
            SERVICE_ACCOUNT_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        assert!(account.users.is_empty());
        let user = account
            .create_new_user("user".to_string())
            .await
            .expect("Failed to add a new user to the test account.");
        assert_eq!(account.users.get(&user.id), Some(&user));
    }

    fn user_generic_pair(n: u128) -> (ID, User, GenericUser) {
        let id = ID::from_u128(n);
        let user = User {
            id: id.clone(),
            username: "test".to_string(),
            created: n,
            in_guilds: Vec::new(),
            parent_id: ACCOUNT_ID.to_string(),
        };
        let generic = GenericUser {
            id: id.clone(),
            username: "test".to_string(),
            created: n,
            guilds_hashed: Vec::new(),
            parent_id: ACCOUNT_ID.to_string(),
        };
        (id, user, generic)
    }

    #[test]
    fn user_to_generic() {
        let uuid = ID::from_u128(0);
        let user = User::new(
            uuid.clone(),
            "Test_with-chars. And".to_string(),
            ACCOUNT_ID.to_string(),
        )
        .expect("Valid username was marked as invalid.");
        let generic = GenericUser {
            id: uuid,
            username: "Test_with-chars. And".to_string(),
            created: user.created,
            parent_id: ACCOUNT_ID.to_string(),
            guilds_hashed: Vec::new(),
        };
        assert_eq!(user.to_generic(), generic);
    }

    #[tokio::test]
    #[serial]
    async fn account_to_generic() {
        let mut account = Account::new(
            SERVICE_ACCOUNT_ID.to_string(),
            EMAIL.to_string(),
            "github".to_string(),
        );
        let user_0 = account
            .create_new_user("user".to_string())
            .await
            .expect("Failed to add a new user to the test account.");
        let user_1 = account
            .create_new_user("user".to_string())
            .await
            .expect("Failed to add a new user to the test account.");
        let user_2 = account
            .create_new_user("user".to_string())
            .await
            .expect("Failed to add a new user to the test account.");
        let mut map: HashMap<ID, GenericUser> = HashMap::new();
        map.insert(user_0.id.clone(), user_0.to_generic().clone());
        map.insert(user_1.id.clone(), user_1.to_generic().clone());
        map.insert(user_2.id.clone(), user_2.to_generic().clone());
        let generic = GenericAccount {
            id: ACCOUNT_ID.to_string(),
            created: account.created,
            users: map,
        };
        assert_eq!(account.to_generic(), generic);
    }

    #[test]
    fn vec_to_generic() {
        let mut map_user: HashMap<ID, User> = HashMap::new();
        let mut map_generic: HashMap<ID, GenericUser> = HashMap::new();
        let set_0 = user_generic_pair(0);
        let set_1 = user_generic_pair(1);
        let set_2 = user_generic_pair(2);
        map_user.insert(set_0.0, set_0.1);
        map_user.insert(set_1.0, set_1.1);
        map_user.insert(set_2.0, set_2.1);
        map_generic.insert(set_0.0, set_0.2);
        map_generic.insert(set_1.0, set_1.2);
        map_generic.insert(set_2.0, set_2.2);
        assert_eq!(Account::users_generic(&map_user), map_generic);
    }

    #[tokio::test]
    #[serial]
    async fn account_save_load() {
        let account = Account::new(
            SERVICE_ACCOUNT_ID.to_string(),
            EMAIL.to_string(),
            "service".to_string(),
        );
        let _delete = std::fs::remove_file("data/accounts/".to_string() + &account.id);
        let _save = account.save().await;
        let loaded = Account::load(&account.id)
            .await
            .expect("Failed to load the test account from disk.");
        assert_eq!(account, loaded);
    }
}
