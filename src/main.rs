use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use wicrs_server::{auth::Auth, config};

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

/// Definition of the HTTP API.
pub mod httpapi;
/// Definition of the WebSocket API.
pub mod websocket;

lazy_static! {
    pub static ref CONFIG: config::Config = config::load_config("config.json");
    pub static ref HEARTBEAT_INTERVAL: Duration =
        Duration::from_millis(CONFIG.ws_hb_interval.clone());
    pub static ref CLIENT_TIMEOUT: Duration =
        Duration::from_millis(CONFIG.ws_client_timeout.clone());
    static ref AUTH: Arc<RwLock<Auth>> =
        Arc::new(RwLock::new(Auth::from_config(&CONFIG.auth_services)));
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    httpapi::server(&CONFIG.address).await
}
