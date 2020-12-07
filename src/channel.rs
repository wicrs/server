use std::str::FromStr;

use tokio::{fs, prelude::*};

use rayon::{prelude::*, str::Lines};

use fs::OpenOptions;

use crate::{get_system_millis, ID};

pub struct Channel {
    pub messages: Vec<Message>,
    pub id: ID,
    pub server_id: ID,
    pub name: String,
    pub created: u128,
}

impl Channel {
    pub async fn new(name: String, id: ID, server_id: ID) -> Result<Self, ()> {
        let new = Self {
            name,
            id,
            server_id,
            messages: Vec::new(),
            created: crate::get_system_millis(),
        };
        if let Ok(_) = new.create_dir().await {
            Ok(new)
        } else {
            Err(())
        }
    }

    pub fn get_folder(&self) -> String {
        format!("data/servers/{}/{}", self.server_id, self.id)
    }

    pub async fn create_dir(&self) -> tokio::io::Result<()> {
        tokio::fs::create_dir_all(self.get_folder()).await
    }

    pub async fn add_message(&mut self, message: Message) -> Result<(), ()> {
        let message_string = &message.to_string();
        if let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(self.get_current_file().await)
            .await
        {
            if let Ok(_) = file
                .write((message_string.to_owned() + "\n").as_bytes())
                .await
            {
                self.messages.push(message);
                Ok(())
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    pub async fn on_all_raw_lines<F: FnMut(Lines) -> ()>(&self, mut action: F) {
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut whole_file = String::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    if let Ok(mut file) = fs::File::open(entry.path()).await {
                        whole_file.clear();
                        if let Ok(_) = file.read_to_string(&mut whole_file).await {
                            let lines = whole_file.par_lines();
                            action(lines)
                        }
                    }
                }
            }
        }
    }

    pub async fn find_messages_containing(&self, string: String) -> Vec<ID> {
        let mut results: Vec<ID> = Vec::new();
        self.on_all_raw_lines(|lines| {
            let mut result: Vec<ID> = lines
                .filter(|l| l.contains(&string))
                .filter_map(|m| {
                    if let Ok(message) = m.parse::<Message>() {
                        if message.content.contains(&string) {
                            Some(message.id)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            result.par_sort_unstable();
            results.append(&mut result);
        })
        .await;
        results
    }

    pub async fn get_message(&self, id: String) -> Option<Message> {
        for message in self.messages.iter() {
            if message.id.to_string() == id {
                return Some(message.clone());
            }
        }
        let id = id.as_str();
        let mut result: Option<Message> = None;
        self.on_all_raw_lines(|lines| {
            let mut results: Vec<Message> = lines
                .filter_map(|l| {
                    if l.starts_with(id) {
                        if let Ok(message) = l.parse::<Message>() {
                            Some(message)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            results.par_sort_unstable_by_key(|m| m.created);
            if let Some(message) = results.first() {
                result = Some(message.clone());
            }
        })
        .await;
        return result;
    }

    pub async fn get_current_file(&mut self) -> String {
        let now = get_system_millis() / 1000 / 60 / 60 / 24;
        self.messages.reverse();
        self.messages.truncate(100);
        self.messages.reverse();
        format!("{}/{}", self.get_folder(), now)
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: ID,
    pub sender: ID,
    pub created: u128,
    pub content: String,
}

impl ToString for Message {
    fn to_string(&self) -> String {
        format!(
            "{},{},{:0>39},{}",
            self.id.to_string(),
            self.sender.to_string(),
            self.created,
            self.content.replace('\n', r#"\n"#)
        )
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
                                        content: content.replace(r#"\n"#, "\n"),
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
