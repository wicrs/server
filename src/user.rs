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
    hub::Hub,
    check_name_validity, Error, Result, ID,
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

    pub async fn change_username(&mut self, new_name: String) -> Result<String> {
        check_name_validity(&new_name)?;
        let old_name = self.username.clone();
        self.username = new_name;
        Ok(old_name)
    }

    pub async fn send_hub_message(&self, hub_id: &ID, channel_id: &ID, message: String) -> Result<ID> {
        if self.in_hubs.contains(hub_id) {
            if let Ok(mut hub) = Hub::load(hub_id).await {
                hub.send_message(&self.id, channel_id, message).await
            } else {
                Err(Error::HubNotFound)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    pub async fn join_hub(&mut self, hub_id: &ID) -> Result<()> {
        if let Ok(mut hub) = Hub::load(hub_id).await {
            if !hub.bans.contains(&self.id) {
                if let Ok(_) = hub.user_join(&self) {
                    if let Ok(()) = hub.save().await {
                        self.in_hubs.push(hub_id.clone());
                        Ok(())
                    } else {
                        Err(Error::WriteFile)
                    }
                } else {
                    Err(Error::GroupNotFound)
                }
            } else {
                Err(Error::Banned)
            }
        } else {
            Err(Error::HubNotFound)
        }
    }

    pub fn in_hub(&self, hub_id: &ID) -> Result<()> {
        if self.in_hubs.contains(hub_id) {
            Ok(())
        } else {
            Err(Error::NotInHub)
        }
    }

    pub async fn leave_hub(&mut self, hub_id: &ID) -> Result<()> {
        if let Some(index) = self.in_hubs.par_iter().position_any(|id| id == hub_id) {
            if let Ok(mut hub) = Hub::load(hub_id).await {
                if let Ok(()) = hub.user_leave(&self) {
                    if let Ok(()) = hub.save().await {
                        self.in_hubs.remove(index);
                        Ok(())
                    } else {
                        Err(Error::WriteFile)
                    }
                } else {
                    Err(Error::GroupNotFound)
                }
            } else {
                Err(Error::HubNotFound)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    pub async fn save(&self) -> Result<()> {
        if let Err(_) = tokio::fs::create_dir_all(USER_FOLDER).await {
            return Err(Error::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = std::fs::write(
                USER_FOLDER.to_owned() + &self.id.to_string() + ".json",
                json,
            ) {
                Ok(result)
            } else {
                Err(Error::WriteFile)
            }
        } else {
            Err(Error::Serialize)
        }
    }

    pub async fn load(id: &ID) -> Result<Self> {
        let filename = USER_FOLDER.to_owned() + &id.to_string() + ".json";
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(Error::HubNotFound);
        }
        if let Ok(json) = tokio::fs::read_to_string(path).await {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(Error::Deserialize)
            }
        } else {
            Err(Error::ReadFile)
        }
    }

    pub async fn load_get_id(id: &str, service: &Service) -> Result<Self> {
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
    async fn send_hub_message() {
        let mut user = User::new(
            SERVICE_USER_ID.to_string(),
            EMAIL.to_string(),
            Service::GitHub,
        );
        let id = crate::api::create_hub(&mut user, "test".to_string())
            .await
            .expect("Failed to create hub.");
        let mut hub = Hub::load(&id).await.expect("Failed to load test hub.");
        let channel = hub
            .new_channel(&user.id, "test_channel".to_string())
            .await
            .expect("Failed to create test channel.");
        hub.save().await.expect("Failed to save test hub.");
        user.send_hub_message(&id, &channel, "test".to_string())
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
