use crate::ID;
use async_graphql::{Enum, SimpleObject};
use parse_display::{Display, FromStr};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Setting for a permission. If all user has permission set to None and it is set to None in all permission groups they are part of it maps to false.
pub type PermissionSetting = Option<bool>;

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, SimpleObject)]
pub struct HubPermissionSet {
    pub permission: HubPermission,
    pub setting: Option<bool>,
}

impl From<(HubPermission, PermissionSetting)> for HubPermissionSet {
    fn from(tup: (HubPermission, PermissionSetting)) -> Self {
        Self {
            permission: tup.0,
            setting: tup.1,
        }
    }
}

#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, SimpleObject)]
pub struct ChannelPermissionSet {
    pub permission: ChannelPermission,
    pub setting: Option<bool>,
    pub channel: ID,
}

impl From<(ChannelPermission, PermissionSetting, ID)> for ChannelPermissionSet {
    fn from(tup: (ChannelPermission, PermissionSetting, ID)) -> Self {
        Self {
            permission: tup.0,
            setting: tup.1,
            channel: tup.2,
        }
    }
}

/// Hub-wide permission, can be all of these except for the `All` permission can be overridden by channel permissions.
#[derive(
    PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, Display, FromStr, Enum,
)]
pub enum HubPermission {
    All,
    ReadChannels,
    ConfigureChannels,
    Administrate,
    CreateChannel,
    DeleteChannel,
    ArrangeChannels,
    WriteInChannels,
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
#[derive(
    PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug, Display, FromStr, Enum,
)]
pub enum ChannelPermission {
    Write,
    Read,
    Configure,
    All,
}

impl From<ChannelPermission> for HubPermission {
    fn from(channel_perm: ChannelPermission) -> Self {
        match channel_perm {
            ChannelPermission::Write => HubPermission::WriteInChannels,
            ChannelPermission::Read => HubPermission::ReadChannels,
            ChannelPermission::Configure => HubPermission::ConfigureChannels,
            ChannelPermission::All => HubPermission::All,
        }
    }
}

/// Map of channel permissions to permission settings.
pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
