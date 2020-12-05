use crate::{get_system_millis, JsonLoadError, JsonSaveError, ID, new_id};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static ACCOUNT_FOLDER: &str = "data/accounts/";

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub bot: bool,
    pub created: u128,
    pub owner_id: String,
    pub in_guilds: Vec<ID>,
}

impl User {
    pub fn new(username: String, bot: bool, owner_id: String) -> Self {
        Self {
            id: new_id(),
            username,
            bot,
            owner_id,
            created: get_system_millis(),
            in_guilds: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Account {
    pub id: String,
    pub email: String,
    pub created: u128,
    pub service: String,
    pub users: Vec<User>,
}

impl Account {
    pub fn new(id: String, email: String, service: String) -> Self {
        Self {
            id: get_id(&id, &service),
            email,
            service,
            users: Vec::new(),
            created: get_system_millis(),
        }
    }

    pub fn save(&self) -> Result<(), JsonSaveError> {
        if let Err(_) = std::fs::create_dir_all(ACCOUNT_FOLDER) {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = std::fs::write(ACCOUNT_FOLDER.to_owned() + &self.id.to_string(), json) {
                Ok(result)
            } else {
                Err(JsonSaveError::WriteFile)
            }
        } else {
            Err(JsonSaveError::Serialize)
        }
    }

    pub fn load(id: &str) -> Result<Self, JsonLoadError> {
        if let Ok(json) = std::fs::read_to_string(ACCOUNT_FOLDER.to_owned() + id) {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(JsonLoadError::Deserialize)
            }
        } else {
            Err(JsonLoadError::ReadFile)
        }
    }

    pub fn load_get_id(id: &str, service: &str) -> Result<Self, JsonLoadError> {
        Self::load(&get_id(id, service))
    }
}

pub fn get_id(id: &str, service: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(id);
    hasher.update(service);
    format!("{:x}", hasher.finalize())
}
