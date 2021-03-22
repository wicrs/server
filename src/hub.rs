use std::collections::{HashMap, HashSet};

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator};
use serde::{Deserialize, Serialize};

use crate::{
    channel::{Channel, Message},
    check_name_validity, check_permission, get_system_millis, new_id,
    permission::{
        ChannelPermission, ChannelPermissions, HubPermission, HubPermissions, PermissionSetting,
    },
    user::User,
    ApiError, DataError, Result, ID,
};

/// Relative path of the folder in which Hub information files (`hub.json`) files are stored.
pub const HUB_INFO_FOLDER: &str = "data/hubs/info/";
/// Relative path of the folder in which Hub data files are stored (channel directories and messages).
pub const HUB_DATA_FOLDER: &str = "data/hubs/data/";

/// Represents a member of a hub that maps to a user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HubMember {
    /// ID of the user that the hub member represents.
    pub user: ID,
    /// Time in milliseconds since Unix Epoch that the user became a member of the hub.
    pub joined: u128,
    /// ID of the hub that this hub member is in.
    pub hub: ID,
    /// Name used by the hub member.
    pub nickname: String,
    /// Groups that the hub member is part of.
    pub groups: Vec<ID>,
    /// Hub permission settings that the hub member has.
    pub hub_permissions: HubPermissions,
    /// Mapping of channel permission settings the hub member has to the channel they apply to.
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl HubMember {
    /// Creates a new hub member based on a user and the ID of the hub they are part of.
    pub fn new(user: &User, hub: ID) -> Self {
        Self {
            nickname: user.username.clone(),
            user: user.id.clone(),
            hub,
            groups: Vec::new(),
            joined: get_system_millis(),
            hub_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    /// Changes the nickname of the hub member while checking that it adheres to the rules set by `crate::is_valid_name`.
    pub fn set_nickname(&mut self, nickname: String) -> Result<()> {
        check_name_validity(&nickname)?;
        self.nickname = nickname;
        Ok(())
    }

    /// Adds the hub member to a permission group.
    pub fn join_group(&mut self, group: &mut PermissionGroup) {
        if !self.groups.contains(&group.id) {
            self.groups.push(group.id.clone());
        }
        if !group.members.contains(&self.user) {
            group.members.push(self.user.clone());
        }
    }

    /// Removes the hub member from a permission group.
    pub fn leave_group(&mut self, group: &mut PermissionGroup) {
        if let Some(index) = self.groups.par_iter().position_any(|id| id == &group.id) {
            self.groups.remove(index);
        }
        if let Some(index) = group.members.par_iter().position_any(|id| id == &self.user) {
            group.members.remove(index);
        }
    }

    /// Sets a hub permission for the hub member.
    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

    /// Sets a channel permission for the hub member in the specified channel.
    pub fn set_channel_permission(
        &mut self,
        channel: &ID,
        permission: ChannelPermission,
        value: PermissionSetting,
    ) {
        let channel_permissions = self
            .channel_permissions
            .entry(*channel)
            .or_insert(HashMap::new());
        channel_permissions.insert(permission, value);
    }

    /// Checks if the hub member has the `HubPermission::All` permission or if they inherit it from a permission group they are in.
    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &Some(true) {
                return true;
            }
        }
        false
    }

    /// Checks if the hub member has the given hub permission or if they inherit it from a permission group they are in.
    pub fn has_permission(&self, permission: HubPermission, hub: &Hub) -> bool {
        if hub.owner == self.user { // If the user is the owner of the hub they are all powerful.
            return true;
        }
        if self.has_all_permissions() { // If the user has the `All` hub permission we do not need to check individual permissions, even for channels.
            return true;
        }
        if let Some(value) = self.hub_permissions.get(&permission) {
            match value {
                &Some(true) => {
                    return true;
                }
                &Some(false) => {
                    return false;
                }
                None => {}
            };
        } else {
            for group in self.groups.iter() {
                if let Some(group) = hub.groups.get(&group) {
                    if group.has_permission(&permission) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Checks if the hub member has the given channel permission in the given channel or if they inherit it from a permission group they are in.
    pub fn has_channel_permission(
        &self,
        channel: &ID,
        permission: &ChannelPermission,
        hub: &Hub,
    ) -> bool {
        if hub.owner == self.user { // If the user is the owner of the hub they are all powerful.
            return true;
        }
        if self.has_all_permissions() { // If the user has the `All` hub permission we do not need to check individual permissions, even for channels.
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel) {
            if let Some(value) = channel.get(&ChannelPermission::All) {
                if value == &Some(true) {
                    return true;
                }
            }
            if let Some(value) = channel.get(permission) {
                match value {
                    &Some(true) => {
                        return true;
                    }
                    &Some(false) => {
                        return false;
                    }
                    None => {
                        if self.has_permission(permission.hub_equivalent(), hub) {
                            return true;
                        }
                    }
                };
            }
        } else {
            for group in self.groups.iter() {
                if let Some(group) = hub.groups.get(&group) {
                    if group.has_channel_permission(channel, permission) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Represents a set of permissions that can be easily given to any hub member.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PermissionGroup {
    /// ID of the group.
    pub id: ID,
    /// Name of the group.
    pub name: String,
    /// Array of the IDs of hub members who are members of the group.
    pub members: Vec<ID>,
    /// Hub permission settings that the group has.
    pub hub_permissions: HubPermissions,
    /// Mapping of channel permission settings the group has to the channel they apply to.
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
    /// Time in milliseconds since Unix Epoch that the group was created.
    pub created: u128,
}

impl PermissionGroup {
    /// Creates a new permission group given a name and an ID.
    pub fn new(name: String, id: ID) -> Self {
        Self {
            created: get_system_millis(),
            id,
            name,
            members: Vec::new(),
            hub_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    /// Adds a hub member to the group, maps to `HubMember::join_group`.
    pub fn add_member(&mut self, user: &mut HubMember) {
        user.join_group(self)
    }

    /// Removes a hub member from the group, maps to `HubMember::leave_group`.
    pub fn remove_member(&mut self, user: &mut HubMember) {
        user.leave_group(self)
    }

    /// Changes the setting of a hub permission for the group.
    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

    /// Changes the setting of a channel permission for a specific channel for the group.
    pub fn set_channel_permission(
        &mut self,
        channel_id: ID,
        permission: ChannelPermission,
        value: PermissionSetting,
    ) {
        let channel_permissions = self
            .channel_permissions
            .entry(channel_id)
            .or_insert(HashMap::new());
        channel_permissions.insert(permission, value);
    }

    /// Checks if the group has the `All` permission.
    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &Some(true) {
                return true;
            }
        }
        false
    }

    /// Checks if the group has a permission.
    pub fn has_permission(&self, permission: &HubPermission) -> bool {
        if self.has_all_permissions() {
            return true;
        }
        if let Some(value) = self.hub_permissions.get(permission) {
            if value == &Some(true) {
                return true;
            }
        }
        return false;
    }

    /// Checks if the group has a permission in a specific channel.
    pub fn has_channel_permission(&self, channel_id: &ID, permission: &ChannelPermission) -> bool {
        if self.has_all_permissions() {
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel_id) {
            if let Some(value) = channel.get(&ChannelPermission::All) {
                if value == &Some(true) {
                    return true;
                }
            }
            if let Some(value) = channel.get(&permission) {
                if value == &Some(true) {
                    return true;
                } else if value == &None {
                    if self.has_permission(&permission.hub_equivalent()) {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

/// Represents a group of users, permission groups and channels.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hub {
    /// Map of channels to their IDs.
    pub channels: HashMap<ID, Channel>,
    /// Map of hub members to their corresponding user's IDs.
    pub members: HashMap<ID, HubMember>,
    /// List of IDs of all users that are banned from the hub.
    pub bans: HashSet<ID>,
    /// List of IDs of all the users who cannot send **any** messages in the hub.
    pub mutes: HashSet<ID>,
    /// ID of the user who owns the hub, also the creator.
    pub owner: ID,
    /// Map of permission groups to their IDs.
    pub groups: HashMap<ID, PermissionGroup>,
    /// ID of the default permission group to be given to new hub members, for now this is always the "everyone" group.
    pub default_group: ID,
    /// Name of the hub.
    pub name: String,
    /// ID of the hub.
    pub id: ID,
    /// Time the hub was created in milliseconds since Unix Epoch.
    pub created: u128,
}

impl Hub {
    /// Creates a new hub given the ID of the user who should be the owner, the name and the ID the hub should have.
    pub fn new(name: String, id: ID, creator: &User) -> Self {
        let creator_id = creator.id.clone();
        let mut everyone = PermissionGroup::new(String::from("everyone"), new_id());
        let mut owner = HubMember::new(creator, id.clone());
        let mut members = HashMap::new();
        let mut groups = HashMap::new();
        owner.join_group(&mut everyone);
        owner.set_permission(HubPermission::All, Some(true));
        members.insert(creator_id.clone(), owner);
        groups.insert(everyone.id.clone(), everyone.clone());
        Self {
            name,
            id,
            groups,
            default_group: everyone.id.clone(),
            owner: creator_id,
            bans: HashSet::new(),
            mutes: HashSet::new(),
            channels: HashMap::new(),
            members,
            created: get_system_millis(),
        }
    }

    /// Creates a new channel checking that the name adheres to the rules set by `crate::is_valid_name` and that the given hub member has permission to do so.
    pub async fn new_channel(&mut self, member_id: &ID, name: String) -> Result<ID> {
        check_name_validity(&name)?;
        let member = self.get_member(member_id)?;
        check_permission!(member, HubPermission::CreateChannel, self);
        let mut id = new_id();
        while self.channels.contains_key(&id) {
            id = new_id();
        }
        let channel = Channel::new(name, id.clone(), self.id.clone());
        channel.create_dir().await?;
        {
            self.get_member_mut(member_id)?.set_channel_permission(
                &channel.id,
                ChannelPermission::ViewChannel,
                Some(true),
            );
        }
        self.channels.insert(id.clone(), channel);
        Ok(id)
    }

    /// Gets a reference to the channel while checking that the given hub member has permission to view it.
    pub fn get_channel(&self, member_id: &ID, channel_id: &ID) -> Result<&Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::ViewChannel, self);
        if let Some(channel) = self.channels.get(channel_id) {
            Ok(channel)
        } else {
            Err(ApiError::ChannelNotFound)
        }
    }

    /// Gets a mutable reference to the channel while checking that the given hub member has permission to view it.
    pub fn get_channel_mut(&mut self, member_id: &ID, channel_id: &ID) -> Result<&mut Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::ViewChannel, self);
        if let Some(channel) = self.channels.get_mut(channel_id) {
            Ok(channel)
        } else {
            Err(ApiError::ChannelNotFound)
        }
    }

    /// Gets a reference to the hub member.
    pub fn get_member(&self, member_id: &ID) -> Result<HubMember> {
        if let Some(member) = self.members.get(member_id) {
            Ok(member.clone())
        } else {
            Err(ApiError::MemberNotFound)
        }
    }

    /// Gets a mutable reference to the hub member.
    pub fn get_member_mut(&mut self, member_id: &ID) -> Result<&mut HubMember> {
        if let Some(member) = self.members.get_mut(member_id) {
            Ok(member)
        } else {
            Err(ApiError::MemberNotFound)
        }
    }

    /// Renames a channel checking that the name adheres to the rules set by `crate::is_valid_name` and that the given hub member has permission to do so and has permission to view the said channel.
    pub async fn rename_channel(
        &mut self,
        user_id: &ID,
        channel_id: &ID,
        name: String,
    ) -> Result<String> {
        check_name_validity(&name)?;
        if let Some(user) = self.members.get(user_id) {
            check_permission!(user, channel_id, ChannelPermission::ViewChannel, self);
            check_permission!(user, channel_id, ChannelPermission::Configure, self);
            if let Some(channel) = self.channels.get_mut(channel_id) {
                let old_name = channel.name.clone();
                channel.name = name;
                Ok(old_name)
            } else {
                Err(ApiError::ChannelNotFound)
            }
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Deletes a channel while checking that the given user has permission to view it and to delete it.
    pub async fn delete_channel(&mut self, user_id: &ID, channel_id: &ID) -> Result<()> {
        if let Some(user) = self.members.get(user_id) {
            check_permission!(user, HubPermission::DeleteChannel, self);
            check_permission!(user, channel_id, ChannelPermission::ViewChannel, self);
            if let Some(_) = self.channels.remove(channel_id) {
                Ok(())
            } else {
                Err(ApiError::ChannelNotFound)
            }
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Sends a message as a user while checking that the user has permission to view the given channel and to write to it.
    pub async fn send_message(
        &mut self,
        user_id: &ID,
        channel_id: &ID,
        message: String,
    ) -> Result<ID> {
        if let Some(member) = self.members.get(&user_id) {
            if !self.mutes.contains(&user_id) {
                check_permission!(member, channel_id, ChannelPermission::ViewChannel, self);
                check_permission!(member, channel_id, ChannelPermission::SendMessage, self);
                if let Some(channel) = self.channels.get_mut(&channel_id) {
                    let id = new_id();
                    let message = Message {
                        id: id.clone(),
                        sender: member.user.clone(),
                        created: get_system_millis(),
                        content: message,
                    };
                    channel.add_message(message).await?;
                    Ok(id)
                } else {
                    Err(ApiError::ChannelNotFound)
                }
            } else {
                Err(ApiError::Muted)
            }
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Gets the file path to be used for storing the hub's data.
    pub fn get_info_path(&self) -> String {
        format!("{}{:x}.json", HUB_INFO_FOLDER, self.id.as_u128())
    }

    /// Gets the path of the directory in which channel folders should be stored.
    pub fn get_data_path(&self) -> String {
        format!("{}{:x}/", HUB_DATA_FOLDER, self.id.as_u128())
    }

    /// Saves the hub's data to disk.
    pub async fn save(&self) -> Result<()> {
        tokio::fs::create_dir_all(HUB_INFO_FOLDER).await?;
        if let Ok(json) = serde_json::to_string(self) {
            tokio::fs::write(self.get_info_path(), json).await?;
            Ok(())
        } else {
            Err(DataError::Serialize.into())
        }
    }

    /// Loads a hub's data given its ID.
    pub async fn load(id: &ID) -> Result<Self> {
        let filename = format!("{}{:x}.json", HUB_INFO_FOLDER, id.as_u128());
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(ApiError::HubNotFound);
        }
        let json = tokio::fs::read_to_string(path).await?;
        if let Ok(result) = serde_json::from_str(&json) {
            Ok(result)
        } else {
            Err(DataError::Deserialize.into())
        }
    }

    /// Adds a user to a hub, creating and returning the resulting hub member.
    pub fn user_join(&mut self, user: &User) -> Result<HubMember> {
        let mut member = HubMember::new(user, self.id.clone());
        if let Some(group) = self.groups.get_mut(&self.default_group) {
            group.add_member(&mut member);
            self.members.insert(member.user.clone(), member.clone());
            Ok(member)
        } else {
            Err(ApiError::GroupNotFound)
        }
    }

    /// Removes a user from a hub.
    pub fn user_leave(&mut self, user: &User) -> Result<()> {
        if let Some(member) = self.members.get_mut(&user.id) {
            if let Some(group) = self.groups.get_mut(&self.default_group) {
                member.leave_group(group);
                self.members.remove(&user.id);
                Ok(())
            } else {
                Err(ApiError::GroupNotFound)
            }
        } else {
            Err(ApiError::NotInHub)
        }
    }

    /// Kicks a user from a hub, forcing them to leave.
    pub async fn kick_user(&mut self, user_id: &ID) -> Result<()> {
        if self.members.contains_key(user_id) {
            let mut user = User::load(user_id).await?;
            self.user_leave(&user)?;
            if let Some(index) = user.in_hubs.par_iter().position_any(|id| id == &self.id) {
                user.in_hubs.remove(index);
            }
            user.save().await?;
            self.members.remove(&user_id);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Kicks the given user and adds them to the banned list.
    pub async fn ban_user(&mut self, user_id: ID) -> Result<()> {
        self.kick_user(&user_id).await?;
        self.bans.insert(user_id);
        Ok(())
    }

    /// Removes the given user from the banned lis.
    pub fn unban_user(&mut self, user_id: &ID) {
        self.bans.remove(user_id);
    }

    /// Adds the given user to the mute list, preventing them from sending messages.
    pub fn mute_user(&mut self, user_id: ID) {
        self.mutes.insert(user_id);
    }

    /// Removes the given user from the mutes list, allowing them to send messages.
    pub fn unmute_user(&mut self, user_id: &ID) {
        self.mutes.remove(user_id);
    }

    /// Gets a list of the channels that the user has permission to view.
    pub fn get_channels_for_user(&self, user_id: &ID) -> Result<HashMap<ID, Channel>> {
        let hub_im = self.clone();
        if let Some(user) = self.members.get(user_id) {
            let mut result = HashMap::new();
            for channel in self.channels.clone() {
                if user.has_channel_permission(&channel.0, &ChannelPermission::ViewChannel, &hub_im)
                {
                    result.insert(channel.0.clone(), channel.1.clone());
                }
            }
            Ok(result)
        } else {
            Err(ApiError::MemberNotFound)
        }
    }

    /// Returns a hub object with only the items that the user is allowed to view (channels).
    pub fn strip(&self, user_id: &ID) -> Result<Self> {
        let mut hub = self.clone();
        hub.channels = self.get_channels_for_user(user_id)?;
        Ok(hub)
    }
}
