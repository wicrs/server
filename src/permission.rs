use crate::{error::ApiError, ID};
#[cfg(feature = "graphql")]
use async_graphql::{Enum, SimpleObject};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, str::FromStr};

/// Setting for a permission. If all user has permission set to None and it is set to None in all permission groups they are part of it maps to false.
pub type PermissionSetting = Option<bool>;

/// Struct that groups a hub permission with a permission setting.
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
#[cfg_attr(feature = "graphql", derive(SimpleObject))]
pub struct HubPermissionSet {
    /// Permission that this permission set is for.
    pub permission: HubPermission,
    /// Setting for the permission.
    pub setting: PermissionSetting,
}

impl From<(HubPermission, PermissionSetting)> for HubPermissionSet {
    fn from(tup: (HubPermission, PermissionSetting)) -> Self {
        Self {
            permission: tup.0,
            setting: tup.1,
        }
    }
}

/// Datastructure that groups a channel permission setting with the channel ID that it is valid in and a permission setting.
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
#[cfg_attr(feature = "graphql", derive(SimpleObject))]
pub struct ChannelPermissionSet {
    /// Permission that this permission set is for.
    pub permission: ChannelPermission,
    /// Setting for the permission.
    pub setting: PermissionSetting,
    /// ID of the channel that this permission setting is for.
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
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
#[cfg_attr(feature = "graphql", derive(Enum))]
pub enum HubPermission {
    All,
    ReadChannels,
    WriteChannels,
    Administrate,
    ManageChannels,
    Mute,
    Unmute,
    Kick,
    Ban,
    Unban,
}

impl Display for HubPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HubPermission::All => "ALL",
            HubPermission::ReadChannels => "READ_CHANNELS",
            HubPermission::WriteChannels => "WRITE_CHANNELS",
            HubPermission::Administrate => "ADMINISTRATE",
            HubPermission::ManageChannels => "MANAGE_CHANNELS",
            HubPermission::Mute => "MUTE",
            HubPermission::Unmute => "UNMUTE",
            HubPermission::Kick => "KICK",
            HubPermission::Ban => "BAN",
            HubPermission::Unban => "UNBAN",
        })
    }
}

impl FromStr for HubPermission {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "ALL" => HubPermission::All,
            "READ_CHANNELS" => HubPermission::ReadChannels,
            "WRITE_CHANNELS" => HubPermission::WriteChannels,
            "ADMINISTRATE" => HubPermission::Administrate,
            "MANAGE_CHANNELS" => HubPermission::ManageChannels,
            "MUTE" => HubPermission::Mute,
            "UNMUTE" => HubPermission::Unmute,
            "KICK" => HubPermission::Kick,
            "BAN" => HubPermission::Ban,
            "UNBAN" => HubPermission::Unban,
            _ => return Err(ApiError::InvalidText),
        })
    }
}

/// Map of hub permissions to permission settings.
pub type HubPermissions = HashMap<HubPermission, PermissionSetting>;

/// Permissions that only apply to channels, override hub permissions.
#[derive(PartialEq, Hash, Eq, Serialize, Deserialize, Clone, Copy, Debug)]
#[cfg_attr(feature = "graphql", derive(Enum))]
pub enum ChannelPermission {
    Write,
    Read,
    Manage,
    All,
}

impl Display for ChannelPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ChannelPermission::Write => "WRITE",
            ChannelPermission::Read => "READ",
            ChannelPermission::Manage => "MANAGE",
            ChannelPermission::All => "ALL",
        })
    }
}

impl FromStr for ChannelPermission {
    type Err = ApiError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "WRITE" => ChannelPermission::Write,
            "READ" => ChannelPermission::Read,
            "MANAGE" => ChannelPermission::Manage,
            "ALL" => ChannelPermission::All,
            _ => return Err(ApiError::InvalidText),
        })
    }
}

impl From<ChannelPermission> for HubPermission {
    fn from(channel_perm: ChannelPermission) -> Self {
        match channel_perm {
            ChannelPermission::Write => HubPermission::WriteChannels,
            ChannelPermission::Read => HubPermission::ReadChannels,
            ChannelPermission::Manage => HubPermission::ManageChannels,
            ChannelPermission::All => HubPermission::All,
        }
    }
}

/// Map of channel permissions to permission settings.
pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
