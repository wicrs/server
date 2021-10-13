use std::{path::Path, str::FromStr};

use chrono::{DateTime, Utc};
use tokio::fs;

use serde::{Deserialize, Serialize};

use fs::OpenOptions;

use crate::{
    error::{ApiError, Error},
    hub::HUB_DATA_FOLDER,
    new_id, Result, ID,
};

use async_graphql::SimpleObject;

/// Text channel, used to group a manage sets of messages.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Channel {
    /// ID of the channel.
    pub id: ID,
    /// ID of the Hub that the channel belongs to.
    pub hub_id: ID,
    /// Description of the channel.
    pub description: String,
    /// Name of the channel.
    pub name: String,
    /// Date the channel was created in milliseconds since Unix Epoch.
    pub created: DateTime<Utc>,
}

impl Channel {
    /// Creates a new channel object based on parameters.
    pub fn new(name: String, id: ID, hub_id: ID) -> Self {
        Self {
            name,
            id,
            hub_id,
            description: String::new(),
            created: Utc::now(),
        }
    }

    /// Get the path of the channel's data folder, used for storing message files.
    pub fn get_folder(&self) -> String {
        format!(
            "{}{:x}/{:x}",
            HUB_DATA_FOLDER,
            self.hub_id.as_u128(),
            self.id.as_u128()
        )
    }

    /// Creates the channel data folder.
    pub async fn create_dir(&self) -> Result {
        tokio::fs::create_dir_all(self.get_folder()).await?;
        Ok(())
    }

    /// Adds a message to the channel, writes it to the file corresponding to the day the message was sent, one file per day of messages, only created if a message is sent that day.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The message file does not exist and could not be created.
    /// * Was unable to write to the message file.
    pub async fn add_message(&self, message: Message) -> Result {
        let path_string = self.get_current_file();
        let path = Path::new(&path_string);
        if path.parent().expect("must have parent").exists() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;
            bincode::serialize_into(file.into_std().await, &message)?;
            Ok(())
        } else {
            Err(Error::ApiError(ApiError::ChannelNotFound))
        }
    }

    pub async fn write_message(message: Message) -> Result {
        Self::new("".to_string(), message.channel_id, message.hub_id)
            .add_message(message)
            .await
    }

    /// Gets the last messages sent, `max` indicates the maximum number of messages to return.
    pub async fn get_last_messages(&self, max: usize) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            files.sort_by_key(|f| f.file_name());
            files.reverse();
            for file in files.iter() {
                if let Ok(file) = fs::File::open(file.path()).await {
                    let mut found = bincode::deserialize_from(file.into_std().await)
                        .into_iter()
                        .collect::<Vec<Message>>();
                    found.sort_by_key(|m| m.created);
                    found.reverse();
                    result.append(&mut found);
                    if result.len() >= max {
                        result.truncate(max);
                        return result;
                    }
                }
            }
        }
        result
    }

    /// Tries to get all the messages listed by their IDs in `ids`. Not guaranteed to return all or any of the wanted messages.
    pub async fn get_messages(&self, ids: Vec<ID>) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = tokio::fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = tokio::fs::read(file.path()).await {
                    let iter = bincode::deserialize::<Message>(&file).into_iter();
                    let mut found: Vec<Message> = iter.filter(|m| ids.contains(&m.id)).collect();
                    result.append(&mut found);
                    if ids.len() == result.len() {
                        return result;
                    }
                }
            }
        }
        result
    }

    /// Gets a set of messages between two times given in milliseconds since Unix Epoch.
    ///
    /// # Arguments
    ///
    /// * `from` - The earliest send time a message can have to be included.
    /// * `to` - The latest send time a message can have to be included.
    /// * `invert` - If true messages are returned in order of newest to oldest if false, oldest to newest, search is also done in that order.
    /// * `max` - The maximum number of messages to return.
    pub async fn get_messages_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
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
            files.sort_by_key(|f| f.file_name());
            if invert {
                files.reverse() // Reverse the order of the list of files to search in the correct direction if `invert` is true.
            }
            let div_from = from.timestamp() / 86400; // Get the day that `from` corresponds to.
            let div_to = to.timestamp() / 86400; // Get the day that `to` corresponds to.
            for file in files.iter().filter(|f| {
                if let Ok(fname) = f.file_name().into_string() {
                    if let Ok(n) = i64::from_str(&fname) {
                        return n >= div_from && n <= div_to; // Check that the file is of a day within the given `to` and `from` times.
                    }
                }
                false
            }) {
                if let Ok(file) = fs::File::open(file.path()).await {
                    let mut filtered = bincode::deserialize_from(file.into_std().await)
                        .into_iter()
                        .filter(|message: &Message| {
                            message.created >= from && message.created <= to
                        })
                        .collect::<Vec<Message>>();
                    if invert {
                        filtered.reverse() // Invert the order of found messages for that file if `invert` is true.
                    }
                    filtered.truncate(max - result.len()); // Remove any extra messages if `max` has been reached.
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
        result
    }

    /// Gets all messages that were sent after the message with the given ID.
    pub async fn get_messages_after(&self, id: ID, max: usize) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = fs::File::open(file.path()).await {
                    let mut iter =
                        bincode::deserialize_from::<std::fs::File, Message>(file.into_std().await)
                            .into_iter();
                    if iter.any(|m| m.id == id) {
                        result.append(&mut iter.collect());
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

    /// Unlimited asynchronus version of [`get_messages_after`] for internal use.
    pub async fn get_all_messages_from(&self, id: ID) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = tokio::fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }

            for file in files.iter() {
                if let Ok(file) = tokio::fs::read(file.path()).await {
                    let mut iter = bincode::deserialize::<Message>(&file).into_iter();
                    if iter.any(|m| m.id == id) {
                        result.append(&mut iter.collect());
                    }
                }
            }
        }
        result
    }

    /// Get the first message with the given ID.
    pub async fn get_message(&self, id: ID) -> Option<Message> {
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = fs::File::open(file.path()).await {
                    if let Some(msg) = bincode::deserialize_from(file.into_std().await)
                        .into_iter()
                        .find(|m: &Message| m.id == id)
                    {
                        return Some(msg);
                    }
                }
            }
        }
        None
    }

    /// Gets the path of the current message file, filename is time in milliseconds from Unix Epoch divided by `86400000` (the number of milliseconds in a day).
    pub fn get_current_file(&self) -> String {
        format!("{}/{}", self.get_folder(), Utc::now().date())
    }
}

/// Represents a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SimpleObject)]
pub struct Message {
    /// ID of the message, not actually guaranteed to be unique due to the performance that could be required to check this for every message sent.
    pub id: ID,
    /// ID of the hub the message was sent in.
    pub hub_id: ID,
    /// ID of the channel the message was sent in.
    pub channel_id: ID,
    /// ID of the user that sent the message.
    pub sender: ID,
    /// Date in milliseconds since Unix Epoch that the message was sent.
    pub created: DateTime<Utc>,
    /// The actual text of the message.
    pub content: String,
}

impl Message {
    pub fn new(sender: ID, content: String, hub_id: ID, channel_id: ID) -> Self {
        Self {
            sender,
            content,
            channel_id,
            hub_id,
            created: Utc::now(),
            id: new_id(),
        }
    }
}
