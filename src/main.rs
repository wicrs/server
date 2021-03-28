use wicrs_server::config;
use wicrs_server::httpapi::server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = config::load_config("config.json");
    server(config).await
}
