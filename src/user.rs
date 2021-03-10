use std::io::Read;

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator};
use serde::{Deserialize, Serialize};
use sha3::{
    digest::{ExtendableOutput, Update},
    Digest, Sha3_256, Shake128,
};

use crate::{
    auth::Service,
    get_system_millis,
    hub::{Hub, HubMember},
    is_valid_username, ApiActionError, JsonLoadError, JsonSaveError, ID, NAME_ALLOWED_CHARS,
};

static USER_FOLDER: &str = "data/users/";

/// Represents a user, keeps track of which accounts it owns and their metadata.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub email: String,
    pub created: u128,
    pub service: Service,
    pub in_hubs: Vec<ID>,
}

/// Represents the publicly available information on a user, (excludes their email address and the service they signed up with) also only includes the generic version of accounts.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenericUser {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub hubs_hashed: Vec<String>,
}

impl User {
    /// Creates a new user and generates an ID by hashing the service used and the ID of the user according to that service.
    pub fn new(id: String, email: String, service: Service) -> Self {
        Self {
            id: get_id(&id, &service),
            username: String::new(),
            email,
            service,
            created: get_system_millis(),
            in_hubs: Vec::new(),
        }
    }

    /// Converts the standard user into a GenericUser.
    pub fn to_generic(&self) -> GenericUser {
        let mut hasher = Sha3_256::new();
        let mut hubs_hashed = Vec::new();
        for hub in self.in_hubs.clone() {
            sha3::digest::Update::update(&mut hasher, hub.to_string());
            hubs_hashed.push(format!("{:x}", hasher.finalize_reset()));
        }
        GenericUser {
            id: self.id.clone(),
            created: self.created.clone(),
            username: self.username.clone(),
            hubs_hashed,
        }
    }

    pub async fn change_username(&mut self, new_name: String) -> Result<String, ApiActionError> {
        if is_valid_username(&new_name) {
            let old_name = self.username.clone();
            self.username = new_name;
            if let Ok(_save) = self.save().await {
                Ok(old_name)
            } else {
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::BadNameCharacters)
        }
    }

    pub async fn send_hub_message(
        &self,
        hub: ID,
        channel: ID,
        message: String,
    ) -> Result<ID, ApiActionError> {
        if self.in_hubs.contains(&hub) {
            if let Ok(mut hub) = Hub::load(hub).await {
                hub.send_message(self.id, channel, message).await
            } else {
                Err(ApiActionError::HubNotFound)
            }
        } else {
            Err(ApiActionError::NotInHub)
        }
    }

    pub async fn join_hub(&mut self, hub_id: ID) -> Result<HubMember, ApiActionError> {
        if let Ok(mut hub) = Hub::load(hub_id).await {
            if !hub.bans.contains(&self.id) {
                if let Ok(member) = hub.user_join(&self) {
                    if let Ok(()) = hub.save().await {
                        self.in_hubs.push(hub_id);
                        if let Ok(()) = self.save().await {
                            Ok(member)
                        } else {
                            Err(ApiActionError::WriteFileError)
                        }
                    } else {
                        Err(ApiActionError::WriteFileError)
                    }
                } else {
                    Err(ApiActionError::GroupNotFound)
                }
            } else {
                Err(ApiActionError::Banned)
            }
        } else {
            Err(ApiActionError::HubNotFound)
        }
    }

    pub async fn leave_hub(&mut self, hub_id: ID) -> Result<(), ApiActionError> {
        if let Some(index) = self.in_hubs.par_iter().position_any(|id| id == &hub_id) {
            if let Ok(mut hub) = Hub::load(hub_id).await {
                if let Ok(()) = hub.user_leave(&self) {
                    if let Ok(()) = hub.save().await {
                        self.in_hubs.remove(index);
                        if let Ok(()) = self.save().await {
                            Ok(())
                        } else {
                            Err(ApiActionError::WriteFileError)
                        }
                    } else {
                        Err(ApiActionError::WriteFileError)
                    }
                } else {
                    Err(ApiActionError::GroupNotFound)
                }
            } else {
                Err(ApiActionError::HubNotFound)
            }
        } else {
            Err(ApiActionError::NotInHub)
        }
    }

    pub async fn create_hub(&mut self, name: String, id: ID) -> Result<ID, ApiActionError> {
        if !name.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
            return Err(ApiActionError::BadNameCharacters);
        }
        if Hub::load(id).await.is_err() {
            let new_hub = Hub::new(name, id, &self);
            if let Ok(_) = new_hub.save().await {
                self.in_hubs.push(new_hub.id.clone());
                if let Ok(_) = self.save().await {
                    Ok(new_hub.id)
                } else {
                    Err(ApiActionError::WriteFileError)
                }
            } else {
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::WriteFileError)
        }
    }

    pub async fn is_in_hub(&self, hub: ID) -> bool {
        self.in_hubs.contains(&hub)
    }

    pub async fn save(&self) -> Result<(), JsonSaveError> {
        if let Err(_) = tokio::fs::create_dir_all(USER_FOLDER).await {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = std::fs::write(
                USER_FOLDER.to_owned() + &self.id.to_string() + ".json",
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

    pub async fn load(id: &ID) -> Result<Self, JsonLoadError> {
        if let Ok(json) =
            tokio::fs::read_to_string(USER_FOLDER.to_owned() + &id.to_string() + ".json").await
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

    pub async fn load_get_id(id: &str, service: &Service) -> Result<Self, JsonLoadError> {
        Self::load(&get_id(id, service)).await
    }
}

pub fn get_id(id: &str, service: &Service) -> ID {
    let mut hasher = Shake128::default();
    hasher.update(id);
    hasher.update(service.to_string());
    let mut bytes = [0; 16];
    hasher
        .finalize_xof()
        .read_exact(&mut bytes)
        .expect("Failed to read the user ID hash");
    ID::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use crate::hub::Hub;

    use super::{get_id, GenericUser, Service, User, ID};

    static USER_ID: &str = "b5aefca491710ba9965c2ef91384210fbf80d2ada056d3229c09912d343ac6b0";
    static SERVICE_USER_ID: &str = "testid";
    static EMAIL: &str = "test@example.com";

    #[test]
    fn id_gen() {
        assert_eq!(
            get_id(SERVICE_USER_ID, &Service::GitHub).to_string(),
            USER_ID.to_string()
        );
    }

    #[test]
    fn new_account() {
        let user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            Service::GitHub,
        );
        assert_eq!(user.id.to_string(), USER_ID.to_string());
    }

    #[test]
    fn user_to_generic() {
        let uuid = ID::from_u128(0);
        let account = User {
            id: uuid,
            username: "Test_with-chars. And".to_string(),
            created: 0,
            service: Service::GitHub,
            in_hubs: Vec::new(),
            email: "test".to_string(),
        };
        let generic = GenericUser {
            id: uuid,
            username: "Test_with-chars. And".to_string(),
            created: account.created,
            hubs_hashed: Vec::new(),
        };
        assert_eq!(account.to_generic(), generic);
    }

    #[tokio::test]
    #[serial]
    async fn user_save_load() {
        let user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            Service::GitHub,
        );
        let _delete = std::fs::remove_file("data/users/".to_string() + &user.id.to_string());
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
            Service::GitHub,
        );
        let _delete = std::fs::remove_file("data/users/".to_string() + &user.id.to_string());
        let id = ID::from_u128(0);
        let _delete = std::fs::remove_file("data/hubs/info/".to_string() + &id.to_string());
        let hub = user
            .create_hub("test_hub".to_string(), id.clone())
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
        let mut user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            Service::GitHub,
        );
        let id = ID::from_u128(0);
        let hub_id = user
            .create_hub("test_hub".to_string(), id.clone())
            .await
            .expect("Failed to create test hub.");
        let mut hub = Hub::load(hub_id).await.expect("Failed to load test hub.");
        let channel = hub
            .new_channel(user.id, "test_channel".to_string())
            .await
            .expect("Failed to create test channel.");
        hub.save().await.expect("Failed to save test hub.");
        user.send_hub_message(hub_id, channel.clone(), "test".to_string())
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
