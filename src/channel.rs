use std::{fmt::Display, str::FromStr};

use tokio::fs;

use serde::{Deserialize, Serialize};

use fs::OpenOptions;

use crate::{error::DataError, hub::HUB_DATA_FOLDER, Result, ID};

/// Text channel, used to group a manage sets of messages.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    pub created: u128,
}

impl Channel {
    /// Creates a new channel object based on parameters.
    pub fn new(name: String, id: ID, hub_id: ID) -> Self {
        Self {
            name,
            id,
            hub_id,
            description: String::new(),
            created: crate::get_system_millis(),
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
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.get_current_file().await)
            .await?;
        bincode::serialize_into(file.into_std().await, &message)
            .map_err(|_| DataError::Serialize)?;
        Ok(())
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

    /// Gets a set of messages between two times given in milliseconds since Unix Epoch.
    ///
    /// # Arguments
    ///
    /// * `from` - The earliest send time a message can have to be included.
    /// * `to` - The latest send time a message can have to be included.
    /// * `invert` - If true messages are returned in order of newest to oldest if false, oldest to newest, search is also done in that order.
    /// * `max` - The maximum number of messages to return.
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
            files.sort_by_key(|f| f.file_name());
            if invert {
                files.reverse() // Reverse the order of the list of files to search in the correct direction if `invert` is true.
            }
            let div_from = from / 86400000; // Get the day that `from` corresponds to.
            let div_to = to / 86400000; // Get the day that `to` corresponds to.
            for file in files.iter().filter(|f| {
                if let Ok(fname) = f.file_name().into_string() {
                    if let Ok(n) = u128::from_str(&fname) {
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

    /// Search for messages that contain a string, if `case_sensitive` is true than the search is case_sensitive, case sensitive search is marginally faster.
    pub async fn find_messages_containing(
        &self,
        string: String,
        case_sensitive: bool,
        max: usize,
    ) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        let mut search = string;
        if !case_sensitive {
            search.make_ascii_uppercase()
        }
        if let Ok(mut dir) = fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = fs::File::open(file.path()).await {
                    let mut filtered = bincode::deserialize_from(file.into_std().await)
                        .into_iter()
                        .filter(|message: &Message| {
                            if case_sensitive {
                                message.content == search
                            } else {
                                message.content.to_ascii_uppercase() == search
                            }
                        })
                        .collect::<Vec<Message>>();
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
    pub async fn get_messages_after(&self, id: &ID, max: usize) -> Vec<Message> {
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
                    if let Some(_) = iter.position(|m| &m.id == id) {
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
    pub async fn async_get_all_messages_from(&self, id: &ID) -> Vec<Message> {
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
                    if let Some(_) = iter.position(|m| {
                        if &m.id == id {
                            result.push(m);
                            true
                        } else {
                            false
                        }
                    }) {
                        result.append(&mut iter.collect());
                    }
                }
            }
        }
        result
    }

    /// Unlimited synchronus version of [`get_messages_after`] for internal use.
    pub fn get_all_messages_from(&self, id: &ID) -> Vec<Message> {
        let mut result: Vec<Message> = Vec::new();
        if let Ok(mut dir) = std::fs::read_dir(self.get_folder()) {
            let mut files = Vec::new();
            while let Some(Ok(entry)) = dir.next() {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = std::fs::read(file.path()) {
                    let mut iter = bincode::deserialize::<Message>(&file).into_iter();
                    if let Some(_) = iter.position(|m| {
                        if &m.id == id {
                            result.push(m);
                            true
                        } else {
                            false
                        }
                    }) {
                        result.append(&mut iter.collect());
                    }
                }
            }
        }
        result
    }

    /// Get the first message with the given ID.
    pub async fn get_message(&self, id: &ID) -> Option<Message> {
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
                        .find(|m: &Message| &m.id == id)
                    {
                        return Some(msg);
                    }
                }
            }
        }
        None
    }

    /// Gets the path of the current message file, filename is time in milliseconds from Unix Epoch divided by `86400000` (the number of milliseconds in a day).
    pub async fn get_current_file(&self) -> String {
        format!(
            "{}/{}",
            self.get_folder(),
            crate::get_system_millis() / 86400000
        )
    }
}

/// Represents a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// ID of the message, not actually guaranteed to be unique due to the performance that could be required to check this for every message sent.
    pub id: ID,
    /// ID of the user that sent the message.
    pub sender: ID,
    /// Date in milliseconds since Unix Epoch that the message was sent.
    pub created: u128,
    /// The actual text of the message.
    pub content: String,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:X},{:X},{:X},{}",
            self.id.as_u128(),
            self.sender.as_u128(),
            self.created,
            self.content.replace('\n', r#"\n"#)
        ))
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
