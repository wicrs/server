use serde::{Deserialize, Serialize};

/// Configuration object for WICRS Server.
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    /// Address to listen on for HTTP requests. (`host:port`)
    pub address: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            address: "127.0.0.1:8080".to_string(),
        }
    }
}

/// Loads the configuration for wicrs_server from `./config.json`. Causes exit with code 1 if the file cannot be found or cannot be deserialized.
pub fn load_config(path: &str) -> Config {
    if let Ok(read) = std::fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<Config>(&read) {
            return config;
        } else {
            warn!(
                "{} does not contain a valid configuration, using defaults...",
                path
            );
        }
    } else {
        warn!("Failed to read {}, using defaults...", path);
    }
    let config = Config::default();
    if std::fs::write(path, &serde_json::to_string_pretty(&config).unwrap()).is_ok() {
        error!("Failed to write default config to {}", path);
    }
    config
}
