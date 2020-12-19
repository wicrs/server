use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub enum PermissionSetting {
    TRUE,
    FALSE,
    NONE,
}

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Debug)]
pub enum HubPermission {
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

pub type HubPermissions = HashMap<HubPermission, PermissionSetting>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Debug)]
pub enum ChannelPermission {
    SendMessage,
    ReadMessage,
    ViewChannel,
    Configure,
    MuteUser,
}

impl ChannelPermission {
    pub fn hub_equivalent(&self) -> HubPermission {
        match self {
            ChannelPermission::SendMessage => HubPermission::SendMessage,
            ChannelPermission::ReadMessage => HubPermission::ReadMessage,
            ChannelPermission::ViewChannel => HubPermission::ViewChannels,
            ChannelPermission::Configure => HubPermission::ConfigureChannels,
            ChannelPermission::MuteUser => HubPermission::MuteUser,
        }
    }
}

pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
