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

pub const HUB_INFO_FOLDER: &str = "data/hubs/info/";
pub const HUB_DATA_FOLDER: &str = "data/hubs/data/";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HubMember {
    pub user: ID,
    pub joined: u128,
    pub hub: ID,
    pub nickname: String,
    pub groups: Vec<ID>,
    pub hub_permissions: HubPermissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl HubMember {
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

    pub fn set_nickname(&mut self, nickname: String) -> Result<()> {
        check_name_validity(&nickname)?;
        self.nickname = nickname;
        Ok(())
    }

    pub fn join_group(&mut self, group: &mut PermissionGroup) {
        if !self.groups.contains(&group.id) {
            self.groups.push(group.id.clone());
        }
        if !group.members.contains(&self.user) {
            group.members.push(self.user.clone());
        }
    }

    pub fn leave_group(&mut self, group: &mut PermissionGroup) {
        if let Some(index) = self.groups.par_iter().position_any(|id| id == &group.id) {
            self.groups.remove(index);
        }
        if let Some(index) = group.members.par_iter().position_any(|id| id == &self.user) {
            group.members.remove(index);
        }
    }

    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

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

    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &Some(true) {
                return true;
            }
        }
        false
    }

    pub fn has_permission(&self, permission: HubPermission, hub: &Hub) -> bool {
        if hub.owner == self.user {
            return true;
        }
        if self.has_all_permissions() {
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

    pub fn has_channel_permission(
        &self,
        channel: &ID,
        permission: &ChannelPermission,
        hub: &Hub,
    ) -> bool {
        if hub.owner == self.user {
            return true;
        }
        if self.has_all_permissions() {
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PermissionGroup {
    pub id: ID,
    pub name: String,
    pub members: Vec<ID>,
    pub hub_permissions: HubPermissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
    pub created: u128,
}

impl PermissionGroup {
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

    pub fn add_member(&mut self, user: &mut HubMember) {
        user.join_group(self)
    }

    pub fn remove_member(&mut self, user: &mut HubMember) {
        user.leave_group(self)
    }

    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

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

    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &Some(true) {
                return true;
            }
        }
        false
    }

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Hub {
    pub channels: HashMap<ID, Channel>,
    pub members: HashMap<ID, HubMember>,
    pub bans: HashSet<ID>,
    pub mutes: HashSet<ID>,
    pub owner: ID,
    pub groups: HashMap<ID, PermissionGroup>,
    pub default_group: ID,
    pub name: String,
    pub id: ID,
    pub created: u128,
}

impl Hub {
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

    pub fn get_channel(&self, member_id: &ID, channel_id: &ID) -> Result<&Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::ViewChannel, self);
        if let Some(channel) = self.channels.get(channel_id) {
            Ok(channel)
        } else {
            Err(ApiError::ChannelNotFound)
        }
    }

    pub fn get_channel_mut(&mut self, member_id: &ID, channel_id: &ID) -> Result<&mut Channel> {
        let member = self.get_member(member_id)?;
        check_permission!(member, channel_id, ChannelPermission::ViewChannel, self);
        if let Some(channel) = self.channels.get_mut(channel_id) {
            Ok(channel)
        } else {
            Err(ApiError::ChannelNotFound)
        }
    }

    pub fn get_member(&self, member_id: &ID) -> Result<HubMember> {
        if let Some(member) = self.members.get(member_id) {
            Ok(member.clone())
        } else {
            Err(ApiError::MemberNotFound)
        }
    }

    pub fn get_member_mut(&mut self, member_id: &ID) -> Result<&mut HubMember> {
        if let Some(member) = self.members.get_mut(member_id) {
            Ok(member)
        } else {
            Err(ApiError::MemberNotFound)
        }
    }

    pub async fn rename_channel(
        &mut self,
        user_id: &ID,
        channel_id: &ID,
        name: String,
    ) -> Result<String> {
        check_name_validity(&name)?;
        if let Some(user) = self.members.get(user_id) {
            check_permission!(user, channel_id, ChannelPermission::ViewChannel, self);
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

    pub async fn send_message(
        &mut self,
        user_id: &ID,
        channel_id: &ID,
        message: String,
    ) -> Result<ID> {
        if let Some(member) = self.members.get(&user_id) {
            if !self.mutes.contains(&user_id) {
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

    pub async fn save(&self) -> Result<()> {
        tokio::fs::create_dir_all(HUB_INFO_FOLDER).await?;
        if let Ok(json) = serde_json::to_string(self) {
            tokio::fs::write(self.get_info_path(), json).await?;
            Ok(())
        } else {
            Err(DataError::Serialize.into())
        }
    }

    pub fn get_info_path(&self) -> String {
        format!("{}{:x}.json", HUB_INFO_FOLDER, self.id.as_u128())
    }

    pub fn get_data_path(&self) -> String {
        format!("{}{:x}/", HUB_DATA_FOLDER, self.id.as_u128())
    }

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

    pub async fn ban_user(&mut self, user_id: ID) -> Result<()> {
        self.kick_user(&user_id).await?;
        self.bans.insert(user_id);
        Ok(())
    }

    pub fn unban_user(&mut self, user_id: &ID) {
        self.bans.remove(user_id);
    }

    pub fn mute_user(&mut self, user_id: ID) {
        self.mutes.insert(user_id);
    }

    pub fn unmute_user(&mut self, user_id: &ID) {
        self.mutes.remove(user_id);
    }

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

    pub fn strip(&self, user_id: &ID) -> Result<Self> {
        let mut hub = self.clone();
        hub.channels = self.get_channels_for_user(user_id)?;
        Ok(hub)
    }
}
