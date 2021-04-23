use std::{
    collections::{HashMap, HashSet},
    mem,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use crate::{
    channel::Channel,
    check_name_validity, check_permission,
    error::Error,
    new_id,
    permission::{
        ChannelPermission, ChannelPermissions, HubPermission, HubPermissions, PermissionSetting,
    },
    Result, ID,
};

/// Relative path of the folder in which Hub information files (`${ID}`) files are stored.
pub const HUB_INFO_FOLDER: &str = "data/hubs/info/";
/// Relative path of the folder in which Hub data files are stored (channel directories and messages).
pub const HUB_DATA_FOLDER: &str = "data/hubs/data/";

/// Represents a member of a hub that maps to a user.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HubMember {
    /// ID of the user that the hub member represents.
    pub user_id: String,
    /// Time in milliseconds since Unix Epoch that the user became a member of the hub.
    pub joined: DateTime<Utc>,
    /// ID of the hub that this hub member is in.
    pub hub: ID,
    /// Groups that the hub member is part of.
    pub groups: Vec<ID>,
    /// Hub permission settings that the hub member has.
    pub hub_permissions: HubPermissions,
    /// Mapping of channel permission settings the hub member has to the channel they apply to.
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl HubMember {
    /// Creates a new hub member based on a user and the ID of the hub they are part of.
    pub fn new(user_id: String, hub: ID) -> Self {
        Self {
            user_id,
            hub,
            groups: Vec::new(),
            joined: Utc::now(),
            hub_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    /// Adds the hub member to a permission group.
    pub fn join_group(&mut self, group: &mut PermissionGroup) {
        if !self.groups.contains(&group.id) {
            self.groups.push(group.id.clone());
        }
        if !group.members.contains(&self.user_id) {
            group.members.push(self.user_id.clone());
        }
    }

    /// Removes the hub member from a permission group.
    pub fn leave_group(&mut self, group: &mut PermissionGroup) {
        if let Some(index) = self.groups.iter().position(|id| id == &group.id) {
            self.groups.remove(index);
        }
        if let Some(index) = group.members.iter().position(|id| id == &self.user_id) {
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
        if hub.owner == self.user_id {
            // If the user is the owner of the hub they are all powerful.
            return true;
        }
        if self.has_all_permissions() {
            // If the user has the `All` hub permission we do not need to check individual permissions, even for channels.
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
        permission: ChannelPermission,
        hub: &Hub,
    ) -> bool {
        if hub.owner == self.user_id {
            // If the user is the owner of the hub they are all powerful.
            return true;
        }
        if self.has_all_permissions() {
            // If the user has the `All` hub permission we do not need to check individual permissions, even for channels.
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel) {
            if let Some(value) = channel.get(&ChannelPermission::All) {
                if value == &Some(true) {
                    return true;
                }
            }
            if let Some(value) = channel.get(&permission) {
                match value {
                    &Some(true) => {
                        return true;
                    }
                    &Some(false) => {
                        return false;
                    }
                    None => {
                        if self.has_permission(permission.into(), hub) {
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
    pub members: Vec<String>,
    /// Hub permission settings that the group has.
    pub hub_permissions: HubPermissions,
    /// Mapping of channel permission settings the group has to the channel they apply to.
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
    /// Time in milliseconds since Unix Epoch that the group was created.
    pub created: DateTime<Utc>,
}

impl PermissionGroup {
    /// Creates a new permission group given a name and an ID.
    pub fn new(name: String, id: ID) -> Self {
        Self {
            created: Utc::now(),
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
    pub fn has_channel_permission(&self, channel_id: &ID, permission: ChannelPermission) -> bool {
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
                    if self.has_permission(&permission.into()) {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

/// Represents a group of users, permission groups and channels.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Hub {
    /// Map of channels to their IDs.
    pub channels: HashMap<ID, Channel>,
    /// Map of hub members to their corresponding user's IDs.
    pub members: HashMap<String, HubMember>,
    /// List of IDs of all users that are banned from the hub.
    pub bans: HashSet<String>,
    /// List of IDs of all the users who cannot send **any** messages in the hub.
    pub mutes: HashSet<String>,
    /// Description of the hub.
    pub description: String,
    /// ID of the user who owns the hub, also the creator.
    pub owner: String,
    /// Map of permission groups to their IDs.
    pub groups: HashMap<ID, PermissionGroup>,
    /// ID of the default permission group to be given to new hub members, for now this is always the "everyone" group.
    pub default_group: ID,
    /// Name of the hub.
    pub name: String,
    /// ID of the hub.
    pub id: ID,
    /// Time the hub was created in milliseconds since Unix Epoch.
    pub created: DateTime<Utc>,
}

impl Hub {
    /// Creates a new hub given the ID of the user who should be the owner, the name and the ID the hub should have.
    pub fn new(name: String, id: ID, creator: String) -> Self {
        let mut everyone = PermissionGroup::new(String::from("everyone"), new_id());
        let mut owner = HubMember::new(creator.clone(), id.clone());
        let mut members = HashMap::new();
        let mut groups = HashMap::new();
        owner.join_group(&mut everyone);
        owner.set_permission(HubPermission::All, Some(true));
        members.insert(creator.clone(), owner);
        groups.insert(everyone.id.clone(), everyone.clone());
        Self {
            name,
            id,
            groups,
            description: String::new(),
            default_group: everyone.id.clone(),
            owner: creator,
            bans: HashSet::new(),
            mutes: HashSet::new(),
            channels: HashMap::new(),
            members,
            created: Utc::now(),
        }
    }

    /// Creates a new channel while checking that the given user has permission to do so.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * Failed to pass [`check_name_validity`].
    /// * The user it not in the hub.
    /// * The user does not have permission create new channels.
    /// * Any of the reasons outlined in [`Channel::create_dir`].
    pub async fn new_channel(&mut self, member_id: &str, name: String) -> Result<ID> {
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
                ChannelPermission::Read,
                Some(true),
            );
        }
        self.channels.insert(id.clone(), channel);
        Ok(id)
    }

    /// Gets a reference to the channel.
    /// Returns an error if the channel could not be found or the user did not have permission to view the channel.
    pub fn get_channel(&self, member_id: &str, channel_id: &ID) -> Result<&Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::Read, self);
        if let Some(channel) = self.channels.get(channel_id) {
            Ok(channel)
        } else {
            Err(Error::ChannelNotFound)
        }
    }

    /// Gets a mutable reference to the channel.
    /// Returns an error if the channel could not be found or the user did not have permission to view the channel.
    pub fn get_channel_mut(&mut self, member_id: &str, channel_id: &ID) -> Result<&mut Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::Read, self);
        if let Some(channel) = self.channels.get_mut(channel_id) {
            Ok(channel)
        } else {
            Err(Error::ChannelNotFound)
        }
    }

    /// Gets a reference to the hub member, returns an error if the member could not be found.
    pub fn get_member(&self, member_id: &str) -> Result<HubMember> {
        if let Some(member) = self.members.get(member_id) {
            Ok(member.clone())
        } else {
            Err(Error::MemberNotFound)
        }
    }

    /// Gets a mutable reference to the hub member, returns an error if the member could not be found.
    pub fn get_member_mut(&mut self, member_id: &str) -> Result<&mut HubMember> {
        if let Some(member) = self.members.get_mut(member_id) {
            Ok(member)
        } else {
            Err(Error::MemberNotFound)
        }
    }

    /// Changes the description of a channel while checking that the given user has permission to do so.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * Description is bigger than [`crate::MAX_DESCRIPTION_SIZE`].
    /// * The user it not in the hub.
    /// * The user does not have permission to view the channel.
    /// * The user does not have permission to configure the channel.
    /// * The channel does not exist.
    pub async fn change_channel_description(
        &mut self,
        user_id: &str,
        channel_id: &ID,
        new_description: String,
    ) -> Result<String> {
        if new_description.as_bytes().len() > crate::MAX_DESCRIPTION_SIZE {
            Err(Error::TooBig)
        } else {
            if let Some(user) = self.members.get(user_id) {
                check_permission!(user, channel_id, ChannelPermission::Read, self);
                check_permission!(user, channel_id, ChannelPermission::Configure, self);
                if let Some(channel) = self.channels.get_mut(channel_id) {
                    Ok(mem::replace(&mut channel.description, new_description))
                } else {
                    Err(Error::ChannelNotFound)
                }
            } else {
                Err(Error::NotInHub)
            }
        }
    }

    /// Renames a channel while checking that the given user has permission to do so.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * Failed to pass [`check_name_validity`].
    /// * The user it not in the hub.
    /// * The user does not have permission to view the channel.
    /// * The user does not have permission to configure the channel.
    /// * The channel does not exist.
    pub async fn rename_channel(
        &mut self,
        user_id: &str,
        channel_id: &ID,
        new_name: String,
    ) -> Result<String> {
        check_name_validity(&new_name)?;
        if let Some(user) = self.members.get(user_id) {
            check_permission!(user, channel_id, ChannelPermission::Read, self);
            check_permission!(user, channel_id, ChannelPermission::Configure, self);
            if let Some(channel) = self.channels.get_mut(channel_id) {
                Ok(mem::replace(&mut channel.name, new_name))
            } else {
                Err(Error::ChannelNotFound)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    /// Deletes a channel while checking that the given user has permission to do so.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user is not in the hub.
    /// * The channel does not exist.
    /// * THe user does not have permission to view the channel.
    /// * The user does not have permission to delete the channel.
    pub async fn delete_channel(&mut self, user_id: &str, channel_id: &ID) -> Result {
        if let Some(user) = self.members.get(user_id) {
            check_permission!(user, HubPermission::DeleteChannel, self);
            check_permission!(user, channel_id, ChannelPermission::Read, self);
            if let Some(_) = self.channels.remove(channel_id) {
                Ok(())
            } else {
                Err(Error::ChannelNotFound)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    /// Sends a message as a user while checking that the user has permission to view the given channel and to write to it.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user is not in the hub.
    /// * THe user is muted and cannot send messages.
    /// * The channel does not exist.
    /// * The user does not have permission to view the channel.
    /// * The user does not have permission to send messages in the channel.
    /// * The message could not be added to the channel for any of the reasons outlined in [`Channel::add_message`].
    pub async fn send_message(
        &mut self,
        user_id: &str,
        channel_id: &ID,
        message: String,
    ) -> Result {
        if let Some(member) = self.members.get(user_id) {
            if !self.mutes.contains(user_id) {
                check_permission!(member, channel_id, ChannelPermission::Read, self);
                check_permission!(member, channel_id, ChannelPermission::Write, self);
                if let Some(channel) = self.channels.get(&channel_id) {
                    channel.add_message(message).await?;
                    Ok(())
                } else {
                    Err(Error::ChannelNotFound)
                }
            } else {
                Err(Error::Muted)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    /// Gets the file path to be used for storing the hub's data.
    pub fn get_info_path(&self) -> String {
        format!("{}{:x}", HUB_INFO_FOLDER, self.id.as_u128())
    }

    /// Gets the path of the directory in which channel folders should be stored.
    pub fn get_data_path(&self) -> String {
        format!("{}{:x}/", HUB_DATA_FOLDER, self.id.as_u128())
    }

    /// Saves the hub's data to disk.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The hub data could not be serialized.
    /// * The hub info folder does not exist and could not be created.
    /// * The data could not be written to the disk.
    pub async fn save(&self) -> Result {
        tokio::fs::create_dir_all(HUB_INFO_FOLDER).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(self.get_info_path())
            .await?;
        let bytes = bincode::serialize(self)?;
        let mut buf: &[u8] = bytes.as_slice();
        file.write_buf(&mut buf).await?;
        file.flush().await?;
        Ok(())
    }

    /// Loads a hub's data given its ID.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * There is no hub with that ID.
    /// * The hub's data file was corrupt and could not be deserialized.
    pub async fn load(id: &ID) -> Result<Self> {
        let filename = format!("{}{:x}", HUB_INFO_FOLDER, id.as_u128());
        let path = std::path::Path::new(&filename);
        if !path.exists() {
            return Err(Error::HubNotFound);
        }
        let mut file = tokio::fs::OpenOptions::new().read(true).open(path).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        Ok(bincode::deserialize(&buf)?)
    }

    /// Adds a user to a hub, creating and returning the resulting hub member.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just this case:
    ///
    /// * The default permission group could not be found.
    pub fn user_join(&mut self, user_id: String) -> Result<HubMember> {
        let mut member = HubMember::new(user_id, self.id.clone());
        if let Some(group) = self.groups.get_mut(&self.default_group) {
            group.add_member(&mut member);
            self.members.insert(member.user_id.clone(), member.clone());
            Ok(member)
        } else {
            Err(Error::GroupNotFound)
        }
    }

    /// Removes the given user from the hub.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user is not in the hub.
    /// * One of the permission groups the user was in could not be found in the hub.
    pub fn user_leave(&mut self, user_id: &str) -> Result {
        if let Some(member) = self.members.get_mut(user_id) {
            if let Some(group) = self.groups.get_mut(&self.default_group) {
                member.leave_group(group);
                self.members.remove(user_id);
                Ok(())
            } else {
                Err(Error::GroupNotFound)
            }
        } else {
            Err(Error::NotInHub)
        }
    }

    /// Kicks the given user from the hub, forcing them to leave.
    ///
    /// # Errors
    ///
    /// This function will return an error in the following situations, but is not
    /// limited to just these cases:
    ///
    /// * The user could not be removed from the hub for any of the reasons outlined in [`Hub::user_leave`].
    /// * The user's data failed to load for any of the reasons outlined in [`User::load`].
    /// * The user's data failed to save for any of the reasons outlined in [`User::save`].
    pub fn kick_user(&mut self, user_id: &str) -> Result {
        if self.members.contains_key(user_id) {
            self.user_leave(user_id)?;
            self.members.remove(user_id);
        }
        Ok(())
    }

    /// Kicks the given user and adds them to the banned list.
    ///
    /// # Errors
    ///
    /// Possible errors outlined by [`Hub::kick_user`].
    pub fn ban_user(&mut self, user_id: String) -> Result {
        self.kick_user(&user_id)?;
        self.bans.insert(user_id);
        Ok(())
    }

    /// Removes the given user from the banned lis.
    pub fn unban_user(&mut self, user_id: &str) {
        self.bans.remove(user_id);
    }

    /// Adds the given user to the mute list, preventing them from sending messages.
    pub fn mute_user(&mut self, user_id: String) {
        self.mutes.insert(user_id);
    }

    /// Removes the given user from the mutes list, allowing them to send messages.
    pub fn unmute_user(&mut self, user_id: &str) {
        self.mutes.remove(user_id);
    }

    /// Gets a list of the channels that the given user has permission to view.
    ///
    /// # Errors
    ///
    /// This function will only return an error if the given user is not in the hub.
    pub fn get_channels_for_user(&self, user_id: &str) -> Result<HashMap<ID, Channel>> {
        let hub_im = self.clone();
        if let Some(user) = self.members.get(user_id) {
            let mut result = HashMap::new();
            for channel in self.channels.clone() {
                if user.has_channel_permission(&channel.0, ChannelPermission::Read, &hub_im) {
                    result.insert(channel.0.clone(), channel.1.clone());
                }
            }
            Ok(result)
        } else {
            Err(Error::MemberNotFound)
        }
    }

    /// Returns a hub object with only the items that the given user is allowed to view.
    /// Only hides channels that the user does not have permission to view.
    ///
    /// # Errors
    ///
    /// Possible errors are outlined by [`Hub::get_channels_for_user`].
    pub fn strip(&self, user_id: &str) -> Result<Self> {
        let mut hub = self.clone();
        hub.channels = self.get_channels_for_user(user_id)?;
        Ok(hub)
    }
}

#[cfg(test)]
mod test {
    use super::{Hub, ID};

    #[tokio::test]
    async fn save_load() {
        let id = ID::nil();
        let mut hub = dbg!(Hub::new("test_hub".to_string(), id.clone(), id.to_string()));
        let _ = tokio::fs::remove_file(&hub.get_info_path()).await;
        hub.new_channel(&id.to_string(), "test_channel".to_string())
            .await
            .expect("Failed to add a channel to the test hub.");
        hub.save().await.expect("Failed to save the hub.");
        Hub::load(&hub.id).await.expect("Failed to load the hub.");
    }
}
