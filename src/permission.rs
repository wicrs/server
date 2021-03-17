use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

pub type PermissionSetting = Option<bool>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
pub enum HubPermission {
    All,
    ViewChannels,
    ConfigureChannels,
    Administrate,
    CreateChannel,
    DeleteChannel,
    ArrangeChannels,
    SendMessage,
    ReadMessage,
    Mute,
    Unmute,
    Invite,
    Kick,
    Ban,
    Unban,
}

impl Display for HubPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

pub type HubPermissions = HashMap<HubPermission, PermissionSetting>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
pub enum ChannelPermission {
    SendMessage,
    ReadMessage,
    ViewChannel,
    Configure,
    All,
}

impl ChannelPermission {
    pub fn hub_equivalent(&self) -> HubPermission {
        match self {
            ChannelPermission::SendMessage => HubPermission::SendMessage,
            ChannelPermission::ReadMessage => HubPermission::ReadMessage,
            ChannelPermission::ViewChannel => HubPermission::ViewChannels,
            ChannelPermission::Configure => HubPermission::ConfigureChannels,
            ChannelPermission::All => HubPermission::All,
        }
    }
}

impl Display for ChannelPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
