use std::collections::HashMap;

use crate::message::Message;

pub struct Channel {
    pub messages: Vec<Message>,
    pub id: usize,
    pub name: String,
    pub created: u128
}

impl Channel {
    pub fn new(name: String, id: usize) -> Self {
        Self {
            name,
            id,
            messages: Vec::new(),
            created: crate::get_system_millis()
        }
    }
}
