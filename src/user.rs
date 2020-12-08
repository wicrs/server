use std::collections::HashMap;

use crate::{ID, JsonLoadError, JsonSaveError, NAME_ALLOWED_CHARS, get_system_millis, guild::Guild, new_id};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static ACCOUNT_FOLDER: &str = "data/accounts/";

#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub id: ID,
    pub username: String,
    pub created: u128,
    pub parent_id: String,
    pub in_guilds: Vec<ID>,
}

impl User {
    pub fn new(username: String, parent_id: String) -> Result<Self, ()> {
        if username.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
            Ok(Self {
                id: new_id(),
                username,
                parent_id,
                created: get_system_millis(),
                in_guilds: Vec::new(),
            })
        } else {
            Err(())
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
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

    pub async fn send_message(&self, user: ID, guild: ID, channel: ID, message: String) -> Result<(), ()> {
        if let Some(user) = self.users.get(&user) {
            if user.in_guilds.contains(&guild) {
                if let Ok(mut guild) = Guild::load(&guild.to_string()).await {
                    return guild.send_message(user.id, channel, message).await;
                }
            }
        }
        return Err(());
    }

    pub async fn save(&self) -> Result<(), JsonSaveError> {
        if let Err(_) = tokio::fs::create_dir_all(ACCOUNT_FOLDER).await {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) =
                std::fs::write(ACCOUNT_FOLDER.to_owned() + &self.id.to_string(), json)
            {
                Ok(result)
            } else {
                Err(JsonSaveError::WriteFile)
            }
        } else {
            Err(JsonSaveError::Serialize)
        }
    }

    pub async fn load(id: &str) -> Result<Self, JsonLoadError> {
        if let Ok(json) = tokio::fs::read_to_string(ACCOUNT_FOLDER.to_owned() + id).await {
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
