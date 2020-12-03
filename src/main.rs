use std::{collections::HashMap, hash::{Hash, Hasher}, time::{SystemTime, UNIX_EPOCH}};
use uuid::Uuid;

mod channel;
mod github;
mod guild;
mod message;
mod permission;
mod user;

#[tokio::main]
async fn main() {
}

pub fn get_system_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

pub type ID = Uuid;
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}
