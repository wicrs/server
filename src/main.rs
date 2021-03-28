use wicrs_server::{config::Config, httpapi::server};

fn load_config(path: &str) -> Config {
    let read = std::fs::read_to_string(path).expect("Failed to load config.json.");
    serde_json::from_str::<Config>(&read)
        .expect("config.json does not contain a valid configuration.")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = load_config("config.json");
    server(config).await
}
