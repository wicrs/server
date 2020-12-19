use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    convert::TryInto,
    sync::Arc,
};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::{
    auth::Auth,
    channel::{Channel, Message},
    get_system_millis, is_valid_username, new_id,
    permission::{
        ChannelPermission, ChannelPermissions, HubPermission, HubPermissions, PermissionSetting,
    },
    unexpected_response,
    user::Account,
    ApiActionError, JsonLoadError, JsonSaveError, ID,
};

static GUILD_INFO_FOLDER: &str = "data/hubs/info";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HubMember {
    pub user: ID,
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
            user: user.id.clone(),
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
        if !rank.members.contains(&self.user) {
            rank.members.push(self.user.clone());
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
        if hub.owner == self.user {
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
        if hub.owner == self.user {
            return true;
        }
        if self.has_all_permissions() {
            return true;
        }
        if let Some(channel) = self.channel_permissions.get(channel) {
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
            created: crate::get_system_millis(),
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
    pub users: HashMap<ID, HubMember>,
    pub bans: HashSet<ID>,
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
        let mut users = HashMap::new();
        let mut ranks = HashMap::new();
        owner.give_rank(&mut everyone);
        owner.set_permission(HubPermission::All, PermissionSetting::TRUE);
        users.insert(creator_id.clone(), owner);
        ranks.insert(everyone.id.clone(), everyone);
        Self {
            name,
            id,
            ranks,
            default_rank: new_id(),
            owner: creator_id,
            bans: HashSet::new(),
            channels: HashMap::new(),
            users,
            created: get_system_millis(),
        }
    }

    pub async fn new_channel(&mut self, user: ID, name: String) -> Result<ID, ApiActionError> {
        if is_valid_username(&name) {
            if let Some(user) = self.users.get(&user) {
                if user
                    .clone()
                    .has_permission(HubPermission::CreateChannel, self)
                {
                    let mut id = new_id();
                    while self.channels.contains_key(&id) {
                        id = new_id();
                    }
                    if let Ok(channel) = Channel::new(name, id.clone(), self.id.clone()).await {
                        self.channels.insert(id.clone(), channel);
                        Ok(id)
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

    pub async fn send_message(
        &mut self,
        user: ID,
        channel: ID,
        message: String,
    ) -> Result<(), ApiActionError> {
        if let Some(user) = self.users.get(&user) {
            if user
                .clone()
                .has_channel_permission(&channel, &ChannelPermission::SendMessage, self)
            {
                if let Some(channel) = self.channels.get_mut(&channel) {
                    let message = Message {
                        id: new_id(),
                        sender: user.user.clone(),
                        created: get_system_millis(),
                        content: message,
                    };
                    channel.add_message(message).await
                } else {
                    Err(ApiActionError::ChannelNotFound)
                }
            } else {
                Err(ApiActionError::NoPermission)
            }
        } else {
            Err(ApiActionError::NotInHub)
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
        self.users.insert(member.user.clone(), member.clone());
        Ok(member)
    }

    pub fn channels(&mut self, user: ID) -> Result<Vec<Channel>, ApiActionError> {
        let hub_im = self.clone();
        if let Some(user) = self.users.get_mut(&user) {
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

#[derive(Deserialize)]
struct HubCreateQuery {
    account: ID,
    name: String,
}

api_get! { (api_v1_create, HubCreateQuery, warp::path("create")) [auth, user, query]
    if user.accounts.contains_key(&query.account) {
        let mut user = user;
        let create = user.create_hub(query.name, new_id(), query.account).await;
        if let Err(err) = create {
            match err {
                ApiActionError::OpenFileError | ApiActionError::WriteFileError => {
                    warp::reply::with_status(
                        "Server could not save the hub data.",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response()
                }

                ApiActionError::UserNotFound => warp::reply::with_status(
                    "Your user does not have an account with that ID.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response(),
                _ => warp::reply::with_status(
                    "The server is doing things that it shouldn't.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response(),
            }
        } else if let Ok(ok) = create {
            warp::reply::with_status(ok.to_string(), StatusCode::OK).into_response()
        } else {
            unexpected_response()
        }
    } else {
        warp::reply::with_status(
            "Your user does not have an account with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

#[derive(Deserialize)]
struct MessageSendQuery {
    user: ID,
    hub: ID,
    channel: ID,
    message: String,
}

api_get! { (api_v1_sendmessage, MessageSendQuery, warp::path("send_message")) [auth, user, query]
    if user.accounts.contains_key(&query.user) {
        if let Err(err) = user
            .send_hub_message(query.user, query.hub, query.channel, query.message)
            .await
        {
            match err {
                ApiActionError::HubNotFound | ApiActionError::NotInHub => {
                    warp::reply::with_status(
                        "You are not in that hub if it exists.",
                        StatusCode::NOT_FOUND,
                    )
                    .into_response()
                }
                ApiActionError::ChannelNotFound | ApiActionError::NoPermission => {
                    warp::reply::with_status(
                        "You do not have permission to access that channel if it exists.",
                        StatusCode::NOT_FOUND,
                    )
                    .into_response()
                }
                ApiActionError::OpenFileError | ApiActionError::WriteFileError => {
                    warp::reply::with_status(
                        "Server could not save your message.",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response()
                }
                ApiActionError::UserNotFound => warp::reply::with_status(
                    "Your user does not have an account with that ID.",
                    StatusCode::NOT_FOUND,
                )
                .into_response(),
                _ => unexpected_response(),
            }
        } else {
            warp::reply::with_status("Message sent successfully.", StatusCode::OK).into_response()
        }
    } else {
        warp::reply::with_status(
            "Your user does not have an account with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

#[derive(Deserialize, Serialize)]
struct ChannelsQuery {
    user: ID,
    hub: ID,
}

api_get! { (api_v1_getchannels, ChannelsQuery, warp::path("channels")) [auth, user, query]
    if user.accounts.contains_key(&query.user) {
        if let Ok(mut hub) = Hub::load(&query.hub.to_string()).await {
            if let Ok(channels) = hub.channels(query.user) {
                warp::reply::json(&channels).into_response()
            } else {
                warp::reply::with_status(
                    "You are not in that hub if it exists.",
                    StatusCode::NOT_FOUND,
                )
                .into_response()
            }
        } else {
            warp::reply::with_status(
                "You are not in that hub if it exists.",
                StatusCode::NOT_FOUND,
            )
            .into_response()
        }
    } else {
        warp::reply::with_status(
            "Your user does not have an account with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

#[derive(Deserialize, Serialize)]
struct LastMessagesQuery {
    user: ID,
    hub: ID,
    channel: ID,
    count: u128,
}

api_get! { (api_v1_getlastmessages, LastMessagesQuery, warp::path("messages")) [auth, user, query]
    if user.accounts.contains_key(&query.user) {
        if let Ok(mut hub) = Hub::load(&query.hub.to_string()).await {
            if let Some(user) = hub.users.get(&query.user) {
                if user.has_channel_permission(&query.channel, &ChannelPermission::ViewChannel, &hub) && user.has_channel_permission(&query.channel, &ChannelPermission::ReadMessage, &hub) {
                    if let Some(channel) = hub.channels.get_mut(&query.channel) {
                        warp::reply::json(&channel.get_last_messages(query.count.try_into().unwrap()).await).into_response()
                    } else {
                        warp::reply::with_status(
                            "You do not have permission to access that channel if it exists.",
                            StatusCode::NOT_FOUND,
                        )
                        .into_response()
                    }
                } else {
                    warp::reply::with_status(
                        "You do not have permission to access that channel if it exists.",
                        StatusCode::NOT_FOUND,
                    )
                    .into_response()
                }
            } else {
                warp::reply::with_status(
                    "You are not in that hub if it exists.",
                    StatusCode::NOT_FOUND,
                )
                .into_response()
            }
        } else {
            warp::reply::with_status(
                "You are not in that hub if it exists.",
                StatusCode::NOT_FOUND,
            )
            .into_response()
        }
    } else {
        warp::reply::with_status(
            "Your user does not have an account with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

pub fn api_v1(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    api_v1_create(auth_manager.clone())
        .or(api_v1_getchannels(auth_manager.clone()))
        .or(api_v1_sendmessage(auth_manager.clone()))
        .or(api_v1_getlastmessages(auth_manager.clone()))
        .boxed()
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
                    .users
                    .get_mut(&member.user)
                    .expect("Failed to get hub member.");
                member_in_hub.set_permission(HubPermission::CreateChannel, PermissionSetting::TRUE);
                member = member_in_hub.clone();
            }
            assert!(!member.has_permission(HubPermission::All, &hub));
            assert!(member.has_permission(HubPermission::CreateChannel, &hub));
        }
        let channel_0 = hub
            .new_channel(member.user.clone(), "test0".to_string())
            .await
            .expect("Failed to create test channel.");
        let _channel_1 = hub
            .new_channel(member.user.clone(), "test1".to_string())
            .await
            .expect("Failed to create test channel.");
        assert!(hub
            .channels(member.user.clone())
            .expect("Failed to get hub channels.")
            .is_empty());

        {
            let member_in_hub = hub
                .users
                .get_mut(&member.user)
                .expect("Failed to get hub member.");
            member_in_hub.set_channel_permission(
                channel_0.clone(),
                ChannelPermission::ViewChannel,
                PermissionSetting::TRUE,
            );
        }
        let get = hub
            .channels(member.user.clone())
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
