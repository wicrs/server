use std::str::FromStr;

use std::io::prelude::*;

use std::fs::OpenOptions;

use crate::{ID, get_system_millis};

pub struct Channel {
    pub messages: String,
    pub id: ID,
    pub server_id: ID,
    pub name: String,
    pub created: u128
}

impl Channel {
    pub async fn new(name: String, id: ID, server_id: ID) -> Result<Self, ()> {
        let new = Self {
            name,
            id,
            server_id,
            messages: String::new(),
            created: crate::get_system_millis()
        };
        if let Ok(_) = new.create_dir().await {
            Ok(new)
        } else {
            Err(())
        }
    }

    pub async fn create_dir(&self) -> tokio::io::Result<()> {
        tokio::fs::create_dir_all(format!("data/servers/{}/{}", self.server_id, self.id)).await
    }

    pub async fn add_message(&mut self, message: Message) -> Result<(),()> {
        let message_string = &message.to_string();
        if let Ok(mut file) = OpenOptions::new().write(true).create(true).append(true).open(format!("data/servers/{}/{}/{}", self.server_id, self.id, get_system_millis() / 1000 / 60 / 60 / 24)) {
            if let Ok(_) = file.write((message_string.to_owned() + "\n").as_bytes()) {
                self.messages.push_str(message_string);
                Ok(())
            } else {
                Err(())
            }
        } else {
            Err(())
        }
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
