use std::sync::Arc;

use futures::lock::Mutex;
use wicrs_server::{auth::Auth, config};

#[allow(unused_imports)]
#[macro_use]
extern crate lazy_static;

pub mod httpapi;

lazy_static! {
    static ref AUTH: Arc<Mutex<Auth>> = Arc::new(Mutex::new(Auth::from_config()));
    pub static ref CONFIG: config::Config = config::load_config();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    httpapi::server(&CONFIG.address).await
}
