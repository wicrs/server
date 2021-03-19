use std::sync::Arc;

use futures::lock::Mutex;
use wicrs_server::{auth::Auth, config};

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

/// Definition of the HTTP API.
pub mod httpapi;

lazy_static! {
    pub static ref CONFIG: config::Config = config::load_config("config.json");
    static ref AUTH: Arc<Mutex<Auth>> = Arc::new(Mutex::new(Auth::from_config(&CONFIG.auth_services)));
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    httpapi::server(&CONFIG.address).await
}
