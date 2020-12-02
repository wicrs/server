use warp::Filter;
use std::time::{SystemTime, UNIX_EPOCH};

mod message;
mod channel;
mod user;
mod guild;
mod permission;

#[tokio::main]
async fn main() {
    
}

pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}
