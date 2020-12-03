use crate::{get_system_millis, JsonLoadError, JsonSaveError, ID};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub bot: bool,
    pub created: u128,
    pub owner_id: ID,
    pub in_guilds: Vec<ID>,
}

#[derive(Serialize, Deserialize)]
pub struct Account {
    pub id: u128,
    pub email: String,
    pub created: u128,
    pub service: String,
    pub users: Vec<User>,
}

impl Account {
    pub fn new(id: u128, email: String, service: String) -> Self {
        Self {
            id,
            email,
            service,
            users: Vec::new(),
            created: get_system_millis(),
        }
    }

    pub fn save(&self) -> Result<(), JsonSaveError> {
        let service_path = "data/accounts/".to_owned() + &self.service + "/";
        if let Err(_) = std::fs::create_dir_all(&service_path) {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = std::fs::write(service_path + &self.id.to_string(), json) {
                Ok(result)
            } else {
                Err(JsonSaveError::WriteFile)
            }
        } else {
            Err(JsonSaveError::Serialize)
        }
    }

    pub fn load(id: String, service: String) -> Result<Self, JsonLoadError> {
        let service_path = "data/accounts/".to_owned() + &service + "/";
        if let Ok(json) = std::fs::read_to_string(service_path + "/" + &id) {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(JsonLoadError::Deserialize)
            }
        } else {
            Err(JsonLoadError::ReadFile)
        }
    }
}
