use crate::{ID, message::Message};

pub struct Channel {
    pub messages: Vec<Message>,
    pub id: ID,
    pub name: String,
    pub created: u128
}

impl Channel {
    pub fn new(name: String, id: ID) -> Self {
        Self {
            name,
            id,
            messages: Vec::new(),
            created: crate::get_system_millis()
        }
    }
}
