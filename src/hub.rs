use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    channel::{Channel, Message},
    get_system_millis, is_valid_username, new_id,
    permission::{
        ChannelPermission, ChannelPermissions, HubPermission, HubPermissions, PermissionSetting,
    },
    user::Account,
    ApiActionError, JsonLoadError, JsonSaveError, ID,
};

pub static GUILD_INFO_FOLDER: &str = "data/hubs/info";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HubMember {
    pub account: ID,
    pub joined: u128,
    pub hub: ID,
    pub nickname: String,
    pub ranks: Vec<ID>,
    pub hub_permissions: HubPermissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl HubMember {
    pub fn new(user: &Account, hub: ID) -> Self {
        Self {
            nickname: user.username.clone(),
            account: user.id.clone(),
            hub,
            ranks: Vec::new(),
            joined: get_system_millis(),
            hub_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    pub fn set_nickname(&mut self, nickname: String) -> Result<(), ()> {
        if is_valid_username(&nickname) {
            self.nickname = nickname;
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn give_rank(&mut self, rank: &mut PermissionGroup) {
        if !self.ranks.contains(&rank.id) {
            self.ranks.push(rank.id.clone());
        }
        if !rank.members.contains(&self.account) {
            rank.members.push(self.account.clone());
        }
    }

    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

    pub fn set_channel_permission(
        &mut self,
        channel: ID,
        permission: ChannelPermission,
        value: PermissionSetting,
    ) {
        let channel_permissions = self
            .channel_permissions
            .entry(channel)
            .or_insert(HashMap::new());
        channel_permissions.insert(permission, value);
    }

    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &PermissionSetting::TRUE {
                return true;
            }
        }
        false
    }

    pub fn has_permission(&self, permission: HubPermission, hub: &Hub) -> bool {
        println!("{:?}", self.hub_permissions);
        if hub.owner == self.account {
            return true;
        }
        if self.has_all_permissions() {
            return true;
        }
        println!("passed all");
        if let Some(value) = self.hub_permissions.get(&permission) {
            match value {
                &PermissionSetting::TRUE => {
                    return true;
                }
                &PermissionSetting::FALSE => {
                    return false;
                }
                PermissionSetting::NONE => {}
            };
        } else {
            for rank in self.ranks.iter() {
                if let Some(rank) = hub.ranks.get(&rank) {
                    if rank.has_permission(&permission) {
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
        if hub.owner == self.account {
            return true;
        }
        if self.has_all_permissions() {
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel) {
            if let Some(value) = channel.get(&ChannelPermission::All) {
                if value == &PermissionSetting::TRUE {
                    return true;
                }
            }
            if let Some(value) = channel.get(permission) {
                match value {
                    &PermissionSetting::TRUE => {
                        return true;
                    }
                    &PermissionSetting::FALSE => {
                        return false;
                    }
                    PermissionSetting::NONE => {
                        if self.has_permission(permission.hub_equivalent(), hub) {
                            return true;
                        }
                    }
                };
            }
        } else {
            for rank in self.ranks.iter() {
                if let Some(rank) = hub.ranks.get(&rank) {
                    if rank.has_channel_permission(channel, permission) {
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
        user.give_rank(self)
    }

    pub fn set_permission(&mut self, permission: HubPermission, value: PermissionSetting) {
        self.hub_permissions.insert(permission, value);
    }

    pub fn set_channel_permission(
        &mut self,
        channel: ID,
        permission: ChannelPermission,
        value: PermissionSetting,
    ) {
        let channel_permissions = self
            .channel_permissions
            .entry(channel)
            .or_insert(HashMap::new());
        channel_permissions.insert(permission, value);
    }

    pub fn has_all_permissions(&self) -> bool {
        if let Some(value) = self.hub_permissions.get(&HubPermission::All) {
            if value == &PermissionSetting::TRUE {
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
            if value == &PermissionSetting::TRUE {
                return true;
            }
        }
        return false;
    }

    pub fn has_channel_permission(&self, channel: &ID, permission: &ChannelPermission) -> bool {
        if self.has_all_permissions() {
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel) {
            if let Some(value) = channel.get(&ChannelPermission::All) {
                if value == &PermissionSetting::TRUE {
                    return true;
                }
            }
            if let Some(value) = channel.get(&permission) {
                if value == &PermissionSetting::TRUE {
                    return true;
                } else if value == &PermissionSetting::NONE {
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
    pub ranks: HashMap<ID, PermissionGroup>,
    pub default_rank: ID,
    pub name: String,
    pub id: ID,
    pub created: u128,
}

impl Hub {
    pub fn new(name: String, id: ID, creator: &Account) -> Self {
        let creator_id = creator.id.clone();
        let mut everyone = PermissionGroup::new(String::from("everyone"), new_id());
        let mut owner = HubMember::new(creator, id.clone());
        let mut members = HashMap::new();
        let mut ranks = HashMap::new();
        owner.give_rank(&mut everyone);
        owner.set_permission(HubPermission::All, PermissionSetting::TRUE);
        members.insert(creator_id.clone(), owner);
        ranks.insert(everyone.id.clone(), everyone);
        Self {
            name,
            id,
            ranks,
            default_rank: new_id(),
            owner: creator_id,
            bans: HashSet::new(),
            mutes: HashSet::new(),
            channels: HashMap::new(),
            members,
            created: get_system_millis(),
        }
    }

    pub async fn new_channel(&mut self, member_id: ID, name: String) -> Result<ID, ApiActionError> {
        if is_valid_username(&name) {
            if let Some(member) = self.members.get(&member_id) {
                if member
                    .clone()
                    .has_permission(HubPermission::CreateChannel, self)
                {
                    let mut id = new_id();
                    while self.channels.contains_key(&id) {
                        id = new_id();
                    }
                    if let Ok(channel) = Channel::new(name, id.clone(), self.id.clone()).await {
                        self.channels.insert(id.clone(), channel);
                        if let Ok(_) = self.save().await {
                            Ok(id)
                        } else {
                            Err(ApiActionError::WriteFileError)
                        }
                    } else {
                        Err(ApiActionError::WriteFileError)
                    }
                } else {
                    Err(ApiActionError::NoPermission)
                }
            } else {
                Err(ApiActionError::NotInHub)
            }
        } else {
            Err(ApiActionError::BadNameCharacters)
        }
    }

    pub async fn rename_channel(
        &mut self,
        user: ID,
        channel: ID,
        name: String,
    ) -> Result<String, ApiActionError> {
        if is_valid_username(&name) {
            if let Some(user) = self.members.get(&user) {
                if user.clone().has_channel_permission(
                    &channel,
                    &ChannelPermission::ViewChannel,
                    self,
                ) {
                    if let Some(channel) = self.channels.get_mut(&channel) {
                        let old_name = channel.name.clone();
                        channel.name = name;
                        if let Ok(_) = self.save().await {
                            Ok(old_name)
                        } else {
                            Err(ApiActionError::WriteFileError)
                        }
                    } else {
                        Err(ApiActionError::ChannelNotFound)
                    }
                } else {
                    Err(ApiActionError::NoPermission)
                }
            } else {
                Err(ApiActionError::NotInHub)
            }
        } else {
            Err(ApiActionError::BadNameCharacters)
        }
    }

    pub async fn delete_channel(&mut self, user: ID, channel: ID) -> Result<(), ApiActionError> {
        if let Some(user) = self.members.get(&user) {
            if user
                .clone()
                .has_permission(HubPermission::DeleteChannel, self)
            {
                if user.clone().has_channel_permission(
                    &channel,
                    &ChannelPermission::ViewChannel,
                    self,
                ) {
                    if let Some(_) = self.channels.remove(&channel) {
                        if let Ok(_) = self.save().await {
                            Ok(())
                        } else {
                            Err(ApiActionError::WriteFileError)
                        }
                    } else {
                        Err(ApiActionError::ChannelNotFound)
                    }
                } else {
                    Err(ApiActionError::NoPermission)
                }
            } else {
                Err(ApiActionError::NoPermission)
            }
        } else {
            Err(ApiActionError::NotInHub)
        }
    }

    pub async fn send_message(
        &mut self,
        account: ID,
        channel: ID,
        message: String,
    ) -> Result<ID, ApiActionError> {
        if !self.mutes.contains(&account) {
            if let Some(member) = self.members.get(&account) {
                if member.clone().has_channel_permission(
                    &channel,
                    &ChannelPermission::SendMessage,
                    self,
                ) {
                    if let Some(channel) = self.channels.get_mut(&channel) {
                        let id = new_id();
                        let message = Message {
                            id: id.clone(),
                            sender: member.account.clone(),
                            created: get_system_millis(),
                            content: message,
                        };
                        channel.add_message(message).await?;
                        Ok(id)
                    } else {
                        Err(ApiActionError::ChannelNotFound)
                    }
                } else {
                    Err(ApiActionError::NoPermission)
                }
            } else {
                Err(ApiActionError::NotInHub)
            }
        } else {
            Err(ApiActionError::Muted)
        }
    }

    pub async fn save(&self) -> Result<(), JsonSaveError> {
        if let Err(_) = tokio::fs::create_dir_all(GUILD_INFO_FOLDER).await {
            return Err(JsonSaveError::Directory);
        }
        if let Ok(json) = serde_json::to_string(self) {
            if let Ok(result) = tokio::fs::write(
                GUILD_INFO_FOLDER.to_owned() + "/" + &self.id.to_string() + ".json",
                json,
            )
            .await
            {
                Ok(result)
            } else {
                Err(JsonSaveError::WriteFile)
            }
        } else {
            Err(JsonSaveError::Serialize)
        }
    }

    pub async fn load(id: &str) -> Result<Self, JsonLoadError> {
        if let Ok(json) =
            tokio::fs::read_to_string(GUILD_INFO_FOLDER.to_owned() + "/" + id + ".json").await
        {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(JsonLoadError::Deserialize)
            }
        } else {
            Err(JsonLoadError::ReadFile)
        }
    }

    pub fn user_join(&mut self, user: &Account) -> Result<HubMember, ()> {
        let mut member = HubMember::new(user, self.id.clone());
        for (id, rank) in self.ranks.iter_mut() {
            if id == &self.default_rank {
                rank.add_member(&mut member);
                break;
            }
        }
        self.members.insert(member.account.clone(), member.clone());
        Ok(member)
    }

    pub fn channels(&mut self, user: ID) -> Result<Vec<Channel>, ApiActionError> {
        let hub_im = self.clone();
        if let Some(user) = self.members.get_mut(&user) {
            let mut result = Vec::new();
            for channel in self.channels.clone() {
                if user.has_channel_permission(&channel.0, &ChannelPermission::ViewChannel, &hub_im)
                {
                    result.push(channel.1.clone());
                }
            }
            Ok(result)
        } else {
            Err(ApiActionError::UserNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        permission::{ChannelPermission, HubPermission, PermissionSetting},
        user::Account,
        ID,
    };

    use super::{Hub, HubMember, PermissionGroup};

    fn get_user_for_test(id: u128) -> Account {
        Account::new(
            ID::from_u128(id),
            "test_user".to_string(),
            "testid".to_string(),
            false,
        )
        .expect("Failed to create a testing user.")
    }

    fn get_hub_for_test() -> Hub {
        Hub::new("test".to_string(), ID::from_u128(1), &get_user_for_test(1))
    }

    #[test]
    fn hub_creator_permissions() {
        let member = HubMember::new(&get_user_for_test(1), ID::from_u128(1));
        let hub = get_hub_for_test();
        assert!(member.has_permission(HubPermission::All, &hub));
    }

    #[test]
    fn hub_permissions() {
        let mut hub = get_hub_for_test();
        let mut member = hub
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test hub.");
        assert!(!member.has_permission(HubPermission::All, &hub));
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        member.set_permission(HubPermission::SendMessage, PermissionSetting::FALSE);
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        member.set_permission(HubPermission::SendMessage, PermissionSetting::NONE);
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        member.set_permission(HubPermission::SendMessage, PermissionSetting::TRUE);
        assert!(member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        member.set_permission(HubPermission::All, PermissionSetting::TRUE);
        assert!(member.has_permission(HubPermission::ReadMessage, &hub));
        assert!(member.has_permission(HubPermission::SendMessage, &hub));
    }

    #[test]
    fn channel_permissions() {
        let mut hub = get_hub_for_test();
        let mut member = hub
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test hub.");
        assert!(!member.has_permission(HubPermission::All, &hub));
        let id = ID::from_u128(0);
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &hub));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &hub));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::FALSE,
        );
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &hub));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &hub));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::NONE,
        );
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &hub));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &hub));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::TRUE,
        );
        assert!(member.has_channel_permission(&id, &ChannelPermission::SendMessage, &hub));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &hub));
        member.set_permission(HubPermission::All, PermissionSetting::TRUE);
        assert!(member.has_channel_permission(&id, &ChannelPermission::SendMessage, &hub));
        assert!(member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &hub));
    }

    #[test]
    fn rank_permissions() {
        let mut hub = get_hub_for_test();
        let mut member = hub
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test hub.");
        let rank = PermissionGroup::new("test_rank".to_string(), ID::from_u128(0));
        hub.ranks.insert(rank.id.clone(), rank.clone());
        member.give_rank(
            hub.ranks
                .get_mut(&rank.id)
                .expect("Failed to get test rank."),
        );
        assert!(!member.has_permission(HubPermission::All, &hub));
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        hub.ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(HubPermission::SendMessage, PermissionSetting::FALSE);
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        hub.ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(HubPermission::SendMessage, PermissionSetting::NONE);
        assert!(!member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        hub.ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(HubPermission::SendMessage, PermissionSetting::TRUE);
        assert!(member.has_permission(HubPermission::SendMessage, &hub));
        assert!(!member.has_permission(HubPermission::ReadMessage, &hub));
        hub.ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(HubPermission::All, PermissionSetting::TRUE);
        assert!(member.has_permission(HubPermission::ReadMessage, &hub));
        assert!(member.has_permission(HubPermission::SendMessage, &hub));
    }

    #[tokio::test]
    async fn channel_view() {
        let mut hub = get_hub_for_test();
        let mut member = hub
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test hub.");
        {
            {
                let member_in_hub = hub
                    .members
                    .get_mut(&member.account)
                    .expect("Failed to get hub member.");
                member_in_hub.set_permission(HubPermission::CreateChannel, PermissionSetting::TRUE);
                member = member_in_hub.clone();
            }
            assert!(!member.has_permission(HubPermission::All, &hub));
            assert!(member.has_permission(HubPermission::CreateChannel, &hub));
        }
        let channel_0 = hub
            .new_channel(member.account.clone(), "test0".to_string())
            .await
            .expect("Failed to create test channel.");
        let _channel_1 = hub
            .new_channel(member.account.clone(), "test1".to_string())
            .await
            .expect("Failed to create test channel.");
        assert!(hub
            .channels(member.account.clone())
            .expect("Failed to get hub channels.")
            .is_empty());

        {
            let member_in_hub = hub
                .members
                .get_mut(&member.account)
                .expect("Failed to get hub member.");
            member_in_hub.set_channel_permission(
                channel_0.clone(),
                ChannelPermission::ViewChannel,
                PermissionSetting::TRUE,
            );
        }
        let get = hub
            .channels(member.account.clone())
            .expect("Failed to get hub channels.");
        assert_eq!(get.len(), 1);
        assert_eq!(get[0].id, channel_0.clone());
    }

    #[tokio::test]
    async fn save_load() {
        let hub = Hub::new(
            "test".to_string(),
            ID::from_u128(1234),
            &get_user_for_test(1),
        );
        let id_str = hub.id.to_string();
        let id_str = id_str.as_str();
        let _remove = tokio::fs::remove_file(
            "data/hubs/info/".to_string() + &ID::from_u128(1234).to_string() + ".json",
        )
        .await;
        hub.save().await.expect("Failed to save hub info.");
        let load = Hub::load(id_str).await.expect("Failed to load hub info.");
        assert_eq!(hub, load);
    }
}
