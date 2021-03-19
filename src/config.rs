use std::process::exit;

use serde::{Deserialize, Serialize};

/// Configuration object for WICRS Server.
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub auth_services: AuthConfigs,
    pub address: String,
    pub show_version: bool,
}

/// Configuration for a generic OAuth service.
#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
}

/// OAuth service configurations.
#[derive(Serialize, Deserialize)]
pub struct AuthConfigs {
    pub github: Option<AuthConfig>,
}

/// Load the configuration from `config.json`.
pub fn load_config(path: &str) -> Config {
    if let Ok(read) = std::fs::read_to_string(path) {
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
