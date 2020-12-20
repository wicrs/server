use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub auth_services: AuthConfigs,
    pub listen: [u8; 4],
    pub port: u16,
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
    serde_json::from_str::<Config>(
        &std::fs::read_to_string("config.json").expect("Failed to read config file."),
    )
    .expect("Failed to parse JSON in config file.")
}
