use std::str::FromStr;

use tokio::{fs, prelude::*};

use rayon::{prelude::*, str::Lines};

use serde::{Deserialize, Serialize};

use fs::OpenOptions;
use uuid::Uuid;

use crate::{get_system_millis, ApiActionError, ID};

static GUILD_DATA_FOLDER: &str = "data/guilds/data";

#[derive(Serialize, Deserialize, Clone)]
pub struct Channel {
    #[serde(skip)]
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
        format!("{}/{}/{}", GUILD_DATA_FOLDER, self.server_id, self.id)
    }

    pub async fn create_dir(&self) -> tokio::io::Result<()> {
        tokio::fs::create_dir_all(self.get_folder()).await
    }

    pub async fn add_message(&mut self, message: Message) -> Result<(), ApiActionError> {
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
                Err(ApiActionError::WriteFileError)
            }
        } else {
            Err(ApiActionError::OpenFileError)
        }
    }

    pub async fn last_n_messages(&self, max: usize) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            files.par_sort_by_key(|f| f.file_name());
            files.reverse();
            let mut whole_file = String::new();
            for file in files.iter() {
                if let Ok(mut file) = fs::File::open(file.path()).await {
                    whole_file.clear();
                    if let Ok(_) = file.read_to_string(&mut whole_file).await {
                        let lines = whole_file.par_lines().collect::<Vec<&str>>();
                        let found = &mut lines
                            .par_iter()
                            .filter_map(|l| {
                                if let Ok(message) = l.parse::<Message>() {
                                    Some(message)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<Message>>();
                        found.par_sort_by_key(|m| m.created);
                        found.reverse();
                        result.append(found);
                        if result.len() >= max {
                            result.truncate(max);
                            return result;
                        }
                    }
                }
            }
        }
        result
    }

    pub async fn get_messages(
        &self,
        from: u128,
        to: u128,
        invert: bool,
        max: usize,
    ) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            files.par_sort_by_key(|f| f.file_name());
            if invert {
                files.reverse()
            }
            let mut whole_file = String::new();
            for file in files.iter() {
                if let Ok(mut file) = fs::File::open(file.path()).await {
                    whole_file.clear();
                    if let Ok(_) = file.read_to_string(&mut whole_file).await {
                        let lines = whole_file.par_lines();
                        let mut filtered: Vec<Message> = lines
                            .filter_map(|l| {
                                let created = l
                                    .splitn(4, ',')
                                    .skip(2)
                                    .next()
                                    .unwrap_or("0")
                                    .parse::<u128>()
                                    .unwrap_or(0);
                                if created >= from && created <= to {
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
                        if invert {
                            filtered.reverse()
                        }
                        filtered.truncate(max - result.len());
                        result.append(&mut filtered);
                        if result.len() >= max {
                            result.truncate(max);
                            return result;
                        }
                    }
                }
            }
        }
        result
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

    pub async fn find_messages_containing(
        &self,
        string: String,
        case_sensitive: bool,
    ) -> Vec<Message> {
        let mut results: Vec<Message> = Vec::new();
        let mut search = string.clone();
        if !case_sensitive {
            search.make_ascii_uppercase()
        }
        self.on_all_raw_lines(|lines| {
            let mut result: Vec<Message> = lines
                .filter_map(|l| {
                    let mut compare_string = l.splitn(4, ',').last().unwrap_or("").to_string();
                    if !case_sensitive {
                        compare_string.make_ascii_uppercase();
                    }
                    if compare_string.contains(&search) {
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
            let results: Vec<Message> = lines
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

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub id: ID,
    pub sender: ID,
    pub created: u128,
    pub content: String,
}

impl ToString for Message {
    fn to_string(&self) -> String {
        format!(
            "{},{},{},{}",
            self.id.as_u128(),
            self.sender.as_u128(),
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
            if let Ok(id) = uuid_from_num_string(id_str) {
                if let Some(sender_str) = parts.next() {
                    if let Ok(sender) = uuid_from_num_string(sender_str) {
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

fn uuid_from_num_string(string: &str) -> Result<Uuid, ()> {
    if let Ok(num) = string.parse::<u128>() {
        Ok(Uuid::from_u128(num))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use uuid::Uuid;

    use super::{Channel, Message};

    fn new_test_message(variation: u128) -> Message {
        Message {
            id: Uuid::from_u128(123456789 + variation),
            sender: Uuid::from_u128(0987654321),
            created: 12222020,
            content: "This is a test message.\nWith a newline.".to_string()
                + &variation.to_string(),
        }
    }

    async fn new_test_channel() -> Channel {
        let _remove = tokio::fs::remove_dir_all("data/guilds/data/00000000-0000-0000-0000-0000075bcd15/00000000-0000-0000-0000-0000075bcd15").await;
        let channel = Channel::new(
            "test".to_string(),
            Uuid::from_u128(123456789),
            Uuid::from_u128(123456789),
        )
        .await
        .expect("Could not create a channel with ID \"123456789\".");
        channel
    }

    #[test]
    fn message_serialize() {
        let message_struct = new_test_message(0);
        let message_string =
            r#"123456789,987654321,12222020,This is a test message.\nWith a newline.0"#.to_string();
        assert_eq!(message_struct.to_string(), message_string);
    }

    #[test]
    fn message_deserialize() {
        let message_struct = new_test_message(0);
        let message_string =
            r#"123456789,987654321,12222020,This is a test message.\nWith a newline.0"#.to_string();
        assert_eq!(Message::from_str(&message_string).unwrap(), message_struct);
    }

    #[tokio::test]
    async fn new_channel() {
        new_test_channel().await;
        assert!(std::path::Path::new("data/guilds/data/00000000-0000-0000-0000-0000075bcd15/00000000-0000-0000-0000-0000075bcd15").exists());
    }

    #[tokio::test]
    #[serial]
    async fn add_message() {
        let mut channel = new_test_channel().await;
        let message = new_test_message(0);
        channel.add_message(message.clone()).await.unwrap();
        let mut file_string = tokio::fs::read_to_string(channel.get_current_file().await)
            .await
            .unwrap();
        if let Some(string) = file_string.strip_suffix('\n') {
            file_string = string.to_string();
        }
        assert_eq!(Message::from_str(&file_string).unwrap(), message);
    }

    #[tokio::test]
    #[serial]
    async fn get_last_n_messages() {
        let mut channel = new_test_channel().await;
        let message_0 = new_test_message(1);
        let message_1 = new_test_message(2);
        let message_2 = new_test_message(3);
        channel.add_message(message_0.clone()).await.unwrap();
        channel.add_message(message_1.clone()).await.unwrap();
        channel.add_message(message_2.clone()).await.unwrap();
        assert_eq!(channel.last_n_messages(2).await, vec![message_2, message_1]);
    }

    #[tokio::test]
    #[serial]
    async fn get_message() {
        let mut channel = new_test_channel().await;
        let message_0 = new_test_message(4);
        let message_1 = new_test_message(5);
        let message_2 = new_test_message(6);
        channel.add_message(message_0.clone()).await.unwrap();
        channel.add_message(message_1.clone()).await.unwrap();
        channel.add_message(message_2.clone()).await.unwrap();
        assert_eq!(
            channel
                .get_message("00000000-0000-0000-0000-0000075bcd19".to_string())
                .await
                .unwrap()
                .content,
            "This is a test message.\nWith a newline.4".to_string()
        );
    }

    #[tokio::test]
    #[serial]
    async fn find_messages_containing() {
        let mut channel = new_test_channel().await;
        let message_0 = new_test_message(7);
        let message_1 = new_test_message(8);
        let message_2 = new_test_message(9);
        channel.add_message(message_0.clone()).await.unwrap();
        channel.add_message(message_1.clone()).await.unwrap();
        channel.add_message(message_2.clone()).await.unwrap();
        assert_eq!(
            channel
                .find_messages_containing("newline.7".to_string(), true)
                .await,
            vec![message_0.clone()]
        );
        assert_eq!(
            channel
                .find_messages_containing("NeWlInE.7".to_string(), true)
                .await,
            vec![]
        );
        assert_eq!(
            channel
                .find_messages_containing("newline.8".to_string(), false)
                .await,
            vec![message_1.clone()]
        );
        assert_eq!(
            channel
                .find_messages_containing("NeWlInE.8".to_string(), false)
                .await,
            vec![message_1.clone()]
        );
        let all = vec![message_0, message_1, message_2];
        assert_eq!(
            channel
                .find_messages_containing("This".to_string(), true)
                .await,
            all.clone()
        );
        assert_eq!(
            channel
                .find_messages_containing("this".to_string(), true)
                .await,
            vec![]
        );
        assert_eq!(
            channel
                .find_messages_containing("This".to_string(), false)
                .await,
            all.clone()
        );
        assert_eq!(
            channel
                .find_messages_containing("this".to_string(), false)
                .await,
            all.clone()
        );
    }

    #[tokio::test]
    #[serial]
    async fn messages_between_dates() {
        let mut channel = new_test_channel().await;
        let message_0 = Message {
            id: Uuid::from_u128(134),
            sender: Uuid::from_u128(0987654321),
            created: 1,
            content: "This is a test message.\nWith a newline.".to_string(),
        };
        let message_1 = Message {
            id: Uuid::from_u128(135),
            sender: Uuid::from_u128(0987654321),
            created: 2,
            content: "This is a test message.\nWith a newline.".to_string(),
        };
        let message_2 = Message {
            id: Uuid::from_u128(136),
            sender: Uuid::from_u128(0987654321),
            created: 3,
            content: "This is a test message.\nWith a newline.".to_string(),
        };
        let message_3 = Message {
            id: Uuid::from_u128(137),
            sender: Uuid::from_u128(0987654321),
            created: 4,
            content: "This is a test message.\nWith a newline.".to_string(),
        };
        channel.add_message(message_0.clone()).await.unwrap();
        channel.add_message(message_1.clone()).await.unwrap();
        channel.add_message(message_2.clone()).await.unwrap();
        channel.add_message(message_3.clone()).await.unwrap();
        assert_eq!(
            channel.get_messages(2, 3, true, 2).await,
            vec![message_2.clone(), message_1.clone()]
        );
        assert_eq!(
            channel.get_messages(2, 3, false, 2).await,
            vec![message_1.clone(), message_2.clone()]
        );
        assert_eq!(channel.get_messages(2, 3, true, 1).await, vec![message_2]);
        assert_eq!(channel.get_messages(2, 3, false, 1).await, vec![message_1]);
    }
}
