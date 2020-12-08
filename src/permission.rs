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
    Administrate,
    CreateChannel,
    DeleteChannel,
    CreateCategory,
    DeleteCategory,
    ArrangeChannels,
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
    ReadMessages,
    ManageChannel,
    EditChannel,
    DeleteMessages,
    MuteUser,
    BypassSlowmode,
}

pub type ChannelPermissions = HashMap<ChannelPermission, PermissionSetting>;
