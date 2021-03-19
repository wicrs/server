use std::str::FromStr;

use tokio::{fs, prelude::*};

use rayon::{prelude::*, str::Lines};

use serde::{Deserialize, Serialize};

use fs::OpenOptions;

use crate::{get_system_millis, hub::HUB_DATA_FOLDER, Result, ID};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Channel {
    pub id: ID,
    pub server_id: ID,
    pub name: String,
    pub created: u128,
}

impl Channel {
    pub fn new(name: String, id: ID, server_id: ID) -> Self {
        Self {
            name,
            id,
            server_id,
            created: crate::get_system_millis(),
        }
    }

    pub fn get_folder(&self) -> String {
        format!(
            "{}{:x}/{:x}",
            HUB_DATA_FOLDER,
            self.server_id.as_u128(),
            self.id.as_u128()
        )
    }

    pub async fn create_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(self.get_folder()).await?;
        Ok(())
    }

    pub async fn add_message(&mut self, message: Message) -> Result<()> {
        let message_string = &message.to_string();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(self.get_current_file().await)
            .await?;
        file.write((message_string.to_owned() + "\n").as_bytes())
            .await?;
        Ok(())
    }

    pub async fn get_last_messages(&self, max: usize) -> Vec<Message> {
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
            let div_from = from / 86400000;
            let div_to = to / 86400000;
            for file in files.iter().filter(|f| {
                if let Ok(fname) = f.file_name().into_string() {
                    if let Ok(n) = u128::from_str(&fname) {
                        return n >= div_from && n <= div_to;
                    }
                }
                false
            }) {
                if let Ok(mut file) = fs::File::open(file.path()).await {
                    whole_file.clear();
                    if let Ok(_) = file.read_to_string(&mut whole_file).await {
                        let lines = whole_file.par_lines();
                        let mut filtered: Vec<Message> = lines
                            .filter_map(|l| {
                                if let Some(created_str) = l.splitn(4, ',').skip(2).next() {
                                    if let Ok(created) = u128::from_str_radix(created_str, 16) {
                                        if created >= from && created <= to {
                                            if let Ok(message) = l.parse::<Message>() {
                                                return Some(message);
                                            }
                                        }
                                    }
                                }
                                None
                            })
                            .collect();
                        if invert {
                            filtered.reverse()
                        }
                        filtered.truncate(max - result.len());
                        result.append(&mut filtered);
                        let len = result.len();
                        if len == max {
                            return result;
                        } else if result.len() > max {
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

    pub async fn get_message(&self, id: &ID) -> Option<Message> {
        let id = format!("{:X}", id.as_u128());
        let mut result: Option<Message> = None;
        self.on_all_raw_lines(|lines| {
            let results: Vec<Message> = lines
                .filter_map(|l| {
                    if l.starts_with(&id) {
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
        let now = get_system_millis() / 86400000;
        format!("{}/{}", self.get_folder(), now)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: ID,
    pub sender: ID,
    pub created: u128,
    pub content: String,
}

impl ToString for Message {
    fn to_string(&self) -> String {
        format!(
            "{:X},{:X},{:X},{}",
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
            if let Ok(id) = ID::from_str(id_str) {
                if let Some(sender_str) = parts.next() {
                    if let Ok(sender) = ID::from_str(sender_str) {
                        if let Some(created_str) = parts.next() {
                            if let Ok(created) = u128::from_str_radix(created_str, 16) {
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
