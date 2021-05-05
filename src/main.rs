use wicrs_server::config::Config;

/// Loads the configuration for wicrs_server from `./config.json`. Causes exit with code 1 if the file cannot be found or cannot be deserialized.
fn load_config(path: &str) -> Config {
    if let Ok(read) = std::fs::read_to_string(path) {
        if let Ok(config) = serde_json::from_str::<wicrs_server::config::Config>(&read) {
            return config;
        } else {
            println!(
                "WARNING: config.json does not contain a valid configuration, using defaults..."
            );
        }
    } else {
        println!("WARNING: Failed to read config.json, using defaults...");
    }
    let config = Config::default();
    std::fs::write(path, &serde_json::to_string_pretty(&config).unwrap())
        .expect("ERROR: Failed to write default config to disk.");
    config
}

/// Main function, loads config and starts a server for the HTTP API.
#[tokio::main]
async fn main() -> wicrs_server::error::Result {
    let config = load_config("config.json");
    std::fs::create_dir_all("data").expect("ERROR: Failed to create data directory.");
    wicrs_server::httpapi::start(config).await
}
