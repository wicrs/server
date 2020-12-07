use std::str::FromStr;

use crate::ID;

pub struct Channel {
    pub messages: String,
    pub id: ID,
    pub name: String,
    pub created: u128
}

impl Channel {
    pub fn new(name: String, id: ID) -> Self {
        Self {
            name,
            id,
            messages: String::new(),
            created: crate::get_system_millis()
        }
    }

    pub fn add_raw_message(&mut self, message: Message) {
        self.messages.push_str(&message.to_string())
    }
}

#[derive(Debug)]
pub struct Message {
    pub id: ID,
    pub sender: ID,
    pub created: u128,
    pub content: String,
}

impl ToString for Message  {
    fn to_string(&self) -> String {
        format!("{},{},{},{}", self.id.to_string(), self.sender.to_string(), self.created, self.content.replace('\n', r#"\n"#))
    }
}

impl FromStr for Message {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(4, ',');
        if let Some(id_str) = parts.next() {
            if let Ok(id) = id_str.parse::<ID>() {
                if let Some(sender_str) = parts.next() {
                    if let Ok(sender) = sender_str.parse::<ID>() {
                        if let Some(created_str) = parts.next() {
                            if let Ok(created) = created_str.parse::<u128>() {
                                if let Some(content) = parts.next() {
                                    return Ok(Self {
                                        id,
                                        sender,
                                        created,
                                        content: content.replace(r#"\,"#, ",").replace(r#"\n"#, "\n")
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        return Err(());
    }
}
