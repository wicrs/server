use parse_display::{Display, FromStr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Setting for a permission. If all user has permission set to None and it is set to None in all permission groups they are part of it maps to false.
pub type PermissionSetting = Option<bool>;

/// Hub-wide permission, can be all of these except for the `All` permission can be overridden by channel permissions.
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, Display, FromStr)]
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

/// Map of hub permissions to permission settings.
pub type HubPermissions = HashMap<HubPermission, PermissionSetting>;

/// Permissions that only apply to channels, override hub permissions.
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, Display, FromStr)]
pub enum ChannelPermission {
    SendMessage,
    ReadMessage,
    ViewChannel,
    Configure,
    All,
}

impl ChannelPermission {
    /// Gets the equivalent `HubPermission` for a `ChannelPermission`.
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

/// Map of channel permissions to permission settings.
pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
