use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::channel::{Channel, Message};
pub use crate::error::{ApiError as Error, ApiResult as Result};
pub use crate::hub::{Hub, HubMember, PermissionGroup};
pub use crate::permission::{ChannelPermission, HubPermission, PermissionSetting};
pub use crate::ID;

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Response<T> {
    Success(T),
    Error(Error),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HttpServerInfo {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpMemberStatus {
    pub member: bool,
    pub banned: bool,
    pub muted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HttpSetPermission {
    pub setting: PermissionSetting,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpHubUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub default_group: Option<ID>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpChannelUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpSendMessage {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpSetNick {
    pub nick: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpLastMessagesQuery {
    pub max: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMessagesBeforeQuery {
    pub to: ID,
    pub max: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMessagesAfterQuery {
    pub from: ID,
    pub max: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMessagesBetweenQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub max: usize,
    pub new_to_old: bool,
}

/// Wraps `is_valid_name` to return a `Result<()>`.
///
/// # Errors
///
/// This function returns an error for any of the following reasons:
///
/// * The name is too big (maximum in bytes defined by [`MAX_NAME_SIZE`]).
pub fn check_name_validity(name: &str) -> Result {
    if is_valid_name(name) {
        Ok(())
    } else {
        Err(Error::InvalidName)
    }
}

/// Checks if a name is valid (not too long and only allowed characters).
pub fn is_valid_name(name: &str) -> bool {
    name.as_bytes().len() <= crate::MAX_NAME_SIZE
}

/// Generates a new random ID.
#[cfg(feature = "uuid-gen")]
pub fn new_id() -> ID {
    uuid::Uuid::new_v4()
}

/// Messages that can be sent to the server by the websocket client
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsClientMessage {
    SubscribeHub {
        hub_id: ID,
    },
    UnsubscribeHub {
        hub_id: ID,
    },
    SubscribeChannel {
        hub_id: ID,
        channel_id: ID,
    },
    UnsubscribeChannel {
        hub_id: ID,
        channel_id: ID,
    },
    StartTyping {
        hub_id: ID,
        channel_id: ID,
    },
    StopTyping {
        hub_id: ID,
        channel_id: ID,
    },
    SendMessage {
        hub_id: ID,
        channel_id: ID,
        message: String,
    },
}

/// Types of updates that trigger [`ServerNotification::HubUpdated`]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsHubUpdateType {
    HubDeleted,
    HubUpdated,
    UserJoined(ID),
    UserLeft(ID),
    UserBanned(ID),
    UserMuted(ID),
    UserUnmuted(ID),
    UserUnbanned(ID),
    UserKicked(ID),
    UserHubPermissionChanged(ID),
    UserChannelPermissionChanged(ID, ID),
    MemberNicknameChanged(ID),
    ChannelCreated(ID),
    ChannelDeleted(ID),
    ChannelUpdated(ID),
}

/// Messages that the server can send to websocket clients.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WsServerMessage {
    Error(Error),
    InvalidCommand,
    NotSigned,
    CommandFailed,
    ChatMessage {
        sender_id: ID,
        hub_id: ID,
        channel_id: ID,
        message_id: ID,
        message: String,
    },
    HubUpdated {
        hub_id: ID,
        update_type: WsHubUpdateType,
    },
    Success,
    UserStartedTyping {
        user_id: ID,
        hub_id: ID,
        channel_id: ID,
    },
    UserStoppedTyping {
        user_id: ID,
        hub_id: ID,
        channel_id: ID,
    },
}
