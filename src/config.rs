use serde::{Deserialize, Serialize};
use crate::JsonLoadError;

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub auth_services: AuthConfigs,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConfigs {
    pub github: Option<AuthConfig>,
}

pub fn load_config() -> Result<Config, JsonLoadError> {
    if let Ok(string) = std::fs::read_to_string("config.json") {
        if let Ok(json) = serde_json::from_str::<Config>(&string) {
            Ok(json)
        } else {
            Err(JsonLoadError::Deserialize)
        }
    } else {
        Err(JsonLoadError::ReadFile)
    }
}
