use std::process::exit;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub auth_services: AuthConfigs,
    pub address: String,
    pub show_version: bool,
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

pub fn load_config() -> Config {
    if let Ok(read) = std::fs::read_to_string("config.json") {
        if let Ok(config) = serde_json::from_str::<Config>(&read) {
            return config;
        } else {
            println!("config.json does not contain a valid configuration.");
            exit(1);
        }
    } else {
        println!("Failed to load config.json.");
        exit(1);
    }
}
