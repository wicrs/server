use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Serialize, Deserialize, Clone)]
pub enum PermissionSetting {
    TRUE,
    FALSE,
    NONE,
}

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone)]
pub enum GuildPermission {
    All,
    ViewChannels,
    ConfigureChannels,
    Administrate,
    CreateChannel,
    DeleteChannel,
    CreateCategory,
    DeleteCategory,
    ArrangeChannels,
    SendMessage,
    ReadMessage,
    MuteUser,
    Invite,
    Kick,
    Ban,
    Mute,
    AddBot,
}

pub type GuildPremissions = HashMap<GuildPermission, PermissionSetting>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone)]
pub enum ChannelPermission {
    SendMessage,
    ReadMessage,
    ViewChannel,
    Configure,
    MuteUser,
}

impl ChannelPermission {
    pub fn guild_equivalent(&self) -> GuildPermission {
        match self {
            ChannelPermission::SendMessage => GuildPermission::SendMessage,
            ChannelPermission::ReadMessage => GuildPermission::ReadMessage,
            ChannelPermission::ViewChannel => GuildPermission::ViewChannels,
            ChannelPermission::Configure => GuildPermission::ConfigureChannels,
            ChannelPermission::MuteUser => GuildPermission::MuteUser,
        }
    }
}

pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
