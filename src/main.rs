use std::process::exit;

use wicrs_server::{config::Config, httpapi::server, websocket::ClientCommand, ID};

fn load_config(path: &str) -> Config {
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = load_config("config.json");
    println!(
        "{}",
        ClientCommand::SendMessage("Hello!".to_string(), ID::nil(), ID::nil())
    );
    server(config).await
}
