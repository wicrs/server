use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub enum PermissionSetting {
    TRUE,
    FALSE,
    NONE,
}

impl Display for PermissionSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Debug)]
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
        f.write_str(&self.to_string())
    }
}

pub type HubPermissions = HashMap<HubPermission, PermissionSetting>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Debug)]
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
        f.write_str(&self.to_string())
    }
}

pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
