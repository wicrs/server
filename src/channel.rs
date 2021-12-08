#[cfg(feature = "server")]
use std::{path::Path, str::FromStr};

#[cfg(feature = "server")]
use tokio::fs;

#[cfg(feature = "server")]
use fs::OpenOptions;
use tokio::fs::read_dir;

use crate::ID;
#[cfg(feature = "server")]
use crate::{
    error::{ApiError, Error},
    hub::HUB_DATA_FOLDER,
    new_id, Result,
};

#[cfg(feature = "server")]
use async_graphql::SimpleObject;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

/// Text channel, used to group a manage sets of messages.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

#[cfg(feature = "server")]
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
            "{}{}/{}",
            HUB_DATA_FOLDER,
            self.hub_id.to_string(),
            self.id.to_string()
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
        let path_string = format!(
            "{}/{}",
            self.get_folder(),
            message
                .created
                .date()
                .signed_duration_since(Utc.timestamp(0, 0).date())
                .num_milliseconds()
        );
        let path = Path::new(&path_string);
        if path.parent().expect("must have parent").exists() {
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await?;
            let file = file.into_std().await;
            bincode::serialize_into(&file, &message)?;
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
                    if let Ok(file_num) = i64::from_str(&entry.file_name().to_string_lossy()) {
                        files.push((file_num, entry))
                    }
                }
            }
            files.sort_by_key(|(n, _)| *n);
            files.reverse();
            for (_, file) in files.iter() {
                let mut found = Vec::new();
                if let Ok(file) = std::fs::File::open(file.path()) {
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        found.push(message);
                    }
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
        if ids.is_empty() {
            return result;
        }
        if let Ok(mut dir) = tokio::fs::read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry)
                }
            }
            for file in files.iter() {
                if let Ok(file) = std::fs::File::open(file.path()) {
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        if ids.contains(&message.id) {
                            result.push(message);
                            if ids.len() == result.len() {
                                return result;
                            }
                        }
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
            let div_from = from.timestamp() / 86400; // Get the day that `from` corresponds to.
            let div_to = to.timestamp() / 86400; // Get the day that `to` corresponds to.
            let mut files = files
                .iter()
                .filter_map(|f| {
                    if let Ok(fname) = f.file_name().into_string() {
                        if let Ok(n) = i64::from_str(&fname) {
                            // Check that the file is of a day within the given `to` and `from` times.
                            if n >= div_from && n <= div_to {
                                return Some((n, f));
                            }
                        }
                    }
                    None
                })
                .collect::<Vec<_>>();
            files.sort_by_key(|(n, _)| *n);
            if invert {
                files.reverse() // Reverse the order of the list of files to search in the correct direction if `invert` is true.
            }
            for (_, file) in files {
                if let Ok(file) = std::fs::File::open(file.path()) {
                    let mut filtered = Vec::new();
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        if message.created >= from && message.created <= to {
                            filtered.push(message);
                        }
                    }
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
                    if let Ok(file_num) = i64::from_str(&entry.file_name().to_string_lossy()) {
                        files.push((file_num, entry))
                    }
                }
            }
            files.sort_by_key(|(n, _)| *n);
            let mut found = false;
            for (_, file) in files.iter() {
                if let Ok(file) = std::fs::File::open(file.path()) {
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        if found {
                            result.push(message);
                            let len = result.len();
                            if len == max {
                                return result;
                            } else if result.len() > max {
                                result.truncate(max);
                                return result;
                            }
                        } else if message.id == id {
                            found = true;
                            result.push(message);
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
                    if let Ok(file_num) = i64::from_str(&entry.file_name().to_string_lossy()) {
                        files.push((file_num, entry))
                    }
                }
            }
            files.sort_by_key(|(n, _)| *n);
            let mut found = false;
            for (_, file) in files.iter() {
                if let Ok(file) = std::fs::File::open(file.path()) {
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        if found {
                            result.push(message);
                        } else if message.id == id {
                            found = true;
                            result.push(message);
                        }
                    }
                }
            }
        }
        result
    }

    /// Get the first message with the given ID.
    pub async fn get_message(&self, id: ID) -> Option<Message> {
        if let Ok(mut dir) = read_dir(self.get_folder()).await {
            let mut files = Vec::new();
            while let Ok(Some(entry)) = dir.next_entry().await {
                if entry.path().is_file() {
                    files.push(entry);
                }
            }
            for file in files.iter() {
                if let Ok(file) = std::fs::File::open(file.path()) {
                    while let Ok(message) = bincode::deserialize_from::<_, Message>(&file) {
                        if message.id == id {
                            return Some(message);
                        }
                    }
                }
            }
        }
        None
    }
}

/// Represents a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "graphql", derive(SimpleObject))]
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

#[cfg(feature = "server")]
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

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use crate::test::*;

    pub fn test_channel(hub: ID) -> Channel {
        let channel = Channel {
            id: *CHANNEL_ID,
            hub_id: hub,
            description: "test channel description".to_string(),
            name: "test".to_string(),
            created: utc(0),
        };
        std::fs::create_dir_all(channel.get_folder())
            .expect("failed to create the channel directory");
        channel
    }

    pub fn test_message(hub: ID) -> Message {
        Message {
            sender: *USER_ID,
            content: "test message".to_string(),
            hub_id: hub,
            channel_id: *CHANNEL_ID,
            created: utc(0),
            id: *MESSAGE_ID,
        }
    }

    pub async fn add_test_messages(hub: ID) -> Vec<Message> {
        let mut messages = Vec::new();
        for i in 0..100u128 {
            let message = Message {
                sender: *USER_ID,
                content: "test message".to_string(),
                hub_id: hub,
                channel_id: *CHANNEL_ID,
                created: utc(i as i64),
                id: ID::from_u128(i),
            };
            messages.push(message.clone());
            Channel::write_message(message)
                .await
                .expect("failed to add a message");
        }
        messages
    }

    #[tokio::test]
    async fn add_get_message() {
        let channel = test_channel(new_id());
        let message = test_message(channel.hub_id);
        channel
            .add_message(message.clone())
            .await
            .expect("failed to add the message");
        let got = channel
            .get_message(message.id)
            .await
            .expect("failed to get the message");
        assert_eq!(got, message);
    }

    #[tokio::test]
    async fn get_message_from_multiple() {
        let channel = test_channel(new_id());
        let messages = add_test_messages(channel.hub_id).await;
        let message = &messages[10];
        assert_eq!(
            message,
            &channel
                .get_message(message.id)
                .await
                .expect("failed to get a message")
        );
    }

    #[tokio::test]
    async fn get_messages_after() {
        let channel = test_channel(new_id());
        let mut messages = add_test_messages(channel.hub_id).await;
        let messages_split_off = messages.split_off(22);
        assert_eq!(
            messages,
            channel
                .get_messages_after(messages.first().unwrap().id, 22)
                .await
        );
        assert_eq!(
            messages_split_off,
            channel
                .get_messages_after(messages_split_off.first().unwrap().id, 79)
                .await
        );
    }

    #[tokio::test]
    async fn get_messages_between() {
        let channel = test_channel(new_id());
        let mut messages = add_test_messages(channel.hub_id).await;
        messages = messages.split_off(16);
        let _ = messages.split_off(64);
        let first = messages.first().unwrap().created;
        let last = messages.last().unwrap().created;
        let mut end = messages.split_off(32);
        assert_eq!(
            messages,
            channel.get_messages_between(first, last, false, 32).await
        );
        messages.append(&mut end);
        messages.reverse();
        messages.truncate(32);
        assert_eq!(
            messages,
            channel.get_messages_between(first, last, true, 32).await
        );
    }

    #[tokio::test]
    async fn get_messages() {
        let channel = test_channel(new_id());
        let mut messages = add_test_messages(channel.hub_id).await;
        messages.truncate(20);
        messages.reverse();
        messages.truncate(10);
        let mut ids = messages.iter().map(|m| m.id).collect::<Vec<_>>();
        ids.push(ID::from_u128(512));
        let got = channel.get_messages(ids).await;
        for m in messages {
            assert!(got.contains(&m));
        }
    }

    #[tokio::test]
    async fn get_last_messages() {
        let channel = test_channel(new_id());
        let mut messages = add_test_messages(channel.hub_id).await;
        messages.reverse();
        messages.truncate(50);
        assert_eq!(messages, channel.get_last_messages(messages.len()).await);
    }
}
