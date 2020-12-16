use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::{
    auth::Auth,
    channel::{Channel, Message},
    get_system_millis, is_valid_username, new_id,
    permission::{
        ChannelPermission, ChannelPermissions, GuildPermission, GuildPremissions, PermissionSetting,
    },
    unexpected_response,
    user::User,
    ApiActionError, JsonLoadError, JsonSaveError, ID,
};

static GUILD_INFO_FOLDER: &str = "data/guilds/info";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GuildMember {
    pub user: ID,
    pub joined: u128,
    pub guild: ID,
    pub nickname: String,
    pub ranks: Vec<ID>,
    pub guild_permissions: GuildPremissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl GuildMember {
    pub fn new(user: &User, guild: ID) -> Self {
        Self {
            nickname: user.username.clone(),
            user: user.id.clone(),
            guild,
            ranks: Vec::new(),
            joined: get_system_millis(),
            guild_permissions: HashMap::new(),
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

    pub fn give_rank(&mut self, rank: &mut Rank) {
        if !self.ranks.contains(&rank.id) {
            self.ranks.push(rank.id.clone());
        }
        if !rank.members.contains(&self.user) {
            rank.members.push(self.user.clone());
        }
    }

    pub fn set_permission(&mut self, permission: GuildPermission, value: PermissionSetting) {
        self.guild_permissions.insert(permission, value);
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
        if let Some(value) = self.guild_permissions.get(&GuildPermission::All) {
            if value == &PermissionSetting::TRUE {
                return true;
            }
        }
        false
    }

    pub fn has_permission(&self, permission: GuildPermission, guild: &Guild) -> bool {
        println!("{:?}", self.guild_permissions);
        if guild.owner == self.user {
            return true;
        }
        if self.has_all_permissions() {
            return true;
        }
        println!("passed all");
        if let Some(value) = self.guild_permissions.get(&permission) {
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
                if let Some(rank) = guild.ranks.get(&rank) {
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
        guild: &Guild,
    ) -> bool {
        if guild.owner == self.user {
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
                        if self.has_permission(permission.guild_equivalent(), guild) {
                            return true;
                        }
                    }
                };
            }
        } else {
            for rank in self.ranks.iter() {
                if let Some(rank) = guild.ranks.get(&rank) {
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
pub struct Rank {
    pub id: ID,
    pub name: String,
    pub members: Vec<ID>,
    pub guild_permissions: GuildPremissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
    pub created: u128,
}

impl Rank {
    pub fn new(name: String, id: ID) -> Self {
        Self {
            created: crate::get_system_millis(),
            id,
            name,
            members: Vec::new(),
            guild_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    pub fn add_member(&mut self, user: &mut GuildMember) {
        user.give_rank(self)
    }

    pub fn set_permission(&mut self, permission: GuildPermission, value: PermissionSetting) {
        self.guild_permissions.insert(permission, value);
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
        if let Some(value) = self.guild_permissions.get(&GuildPermission::All) {
            if value == &PermissionSetting::TRUE {
                return true;
            }
        }
        false
    }

    pub fn has_permission(&self, permission: &GuildPermission) -> bool {
        if self.has_all_permissions() {
            return true;
        }
        if let Some(value) = self.guild_permissions.get(permission) {
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
                    if self.has_permission(&permission.guild_equivalent()) {
                        return true;
                    }
                }
            }
        }
        return false;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Guild {
    pub channels: HashMap<ID, Channel>,
    pub users: HashMap<ID, GuildMember>,
    pub bans: HashSet<ID>,
    pub owner: ID,
    pub ranks: HashMap<ID, Rank>,
    pub default_rank: ID,
    pub name: String,
    pub id: ID,
    pub created: u128,
}

impl Guild {
    pub fn new(name: String, id: ID, creator: &User) -> Self {
        let creator_id = creator.id.clone();
        let mut everyone = Rank::new(String::from("everyone"), new_id());
        let mut owner = GuildMember::new(creator, id.clone());
        let mut users = HashMap::new();
        let mut ranks = HashMap::new();
        owner.give_rank(&mut everyone);
        owner.set_permission(GuildPermission::All, PermissionSetting::TRUE);
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
                    .has_permission(GuildPermission::CreateChannel, self)
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
                Err(ApiActionError::NotInGuild)
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
            Err(ApiActionError::NotInGuild)
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

    pub fn user_join(&mut self, user: &User) -> Result<GuildMember, ()> {
        let mut member = GuildMember::new(user, self.id.clone());
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
        let guild_im = self.clone();
        if let Some(user) = self.users.get_mut(&user) {
            let mut result = Vec::new();
            for channel in self.channels.clone() {
                if user.has_channel_permission(
                    &channel.0,
                    &ChannelPermission::ViewChannel,
                    &guild_im,
                ) {
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
struct GuildCreateQuery {
    user: ID,
    name: String,
}

api_get! { (api_v1_create, GuildCreateQuery, warp::path("create")) [auth, account, query]
    if account.users.contains_key(&query.user) {
        let mut account = account;
        let create = account.create_guild(query.name, new_id(), query.user).await;
        if let Err(err) = create {
            match err {
                ApiActionError::OpenFileError | ApiActionError::WriteFileError => {
                    warp::reply::with_status(
                        "Server could not save the guild data.",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                    .into_response()
                }

                ApiActionError::UserNotFound => warp::reply::with_status(
                    "That user does not exist on your account.",
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
            "Your account does not have a user with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

#[derive(Deserialize)]
struct MessageSendQuery {
    user: ID,
    guild: ID,
    channel: ID,
    message: String,
}

api_get! { (api_v1_sendmessage, MessageSendQuery, warp::path("send_message")) [auth, account, query]
    if account.users.contains_key(&query.user) {
        if let Err(err) = account
            .send_guild_message(query.user, query.guild, query.channel, query.message)
            .await
        {
            match err {
                ApiActionError::GuildNotFound | ApiActionError::NotInGuild => {
                    warp::reply::with_status(
                        "You are not in that guild if it exists.",
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
                    "That user does not exist on your account.",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
                .into_response(),
                _ => unexpected_response(),
            }
        } else {
            warp::reply::with_status("Message sent successfully.", StatusCode::OK).into_response()
        }
    } else {
        warp::reply::with_status(
            "Your account does not have a user with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

#[derive(Deserialize, Serialize)]
struct ChannelsQuery {
    user: ID,
    guild: ID,
}

api_get! { (api_v1_getchannels, ChannelsQuery, warp::path("channels")) [auth, account, query]
    if account.users.contains_key(&query.user) {
        if let Ok(mut guild) = Guild::load(&query.guild.to_string()).await {
            if let Ok(channels) = guild.channels(query.user) {
                warp::reply::json(&channels).into_response()
            } else {
                warp::reply::with_status(
                    "You are not in that guild if it exists.",
                    StatusCode::NOT_FOUND,
                )
                .into_response()
            }
        } else {
            warp::reply::with_status(
                "You are not in that guild if it exists.",
                StatusCode::NOT_FOUND,
            )
            .into_response()
        }
    } else {
        warp::reply::with_status(
            "Your account does not have a user with that ID.",
            StatusCode::NOT_FOUND,
        )
        .into_response()
    }
}

pub fn api_v1(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    api_v1_create(auth_manager.clone())
        .or(api_v1_getchannels(auth_manager.clone()))
        .or(api_v1_sendmessage(auth_manager.clone()))
        .boxed()
}

#[cfg(test)]
mod tests {
    use crate::{
        permission::{ChannelPermission, GuildPermission, PermissionSetting},
        user::User,
        ID,
    };

    use super::{Guild, GuildMember, Rank};

    fn get_user_for_test(id: u128) -> User {
        User::new(
            ID::from_u128(id),
            "test_user".to_string(),
            "testid".to_string(),
        )
        .expect("Failed to create a testing user.")
    }

    fn get_guild_for_test() -> Guild {
        Guild::new("test".to_string(), ID::from_u128(1), &get_user_for_test(1))
    }

    #[test]
    fn guild_creator_permissions() {
        let member = GuildMember::new(&get_user_for_test(1), ID::from_u128(1));
        let guild = get_guild_for_test();
        assert!(member.has_permission(GuildPermission::All, &guild));
    }

    #[test]
    fn guild_permissions() {
        let mut guild = get_guild_for_test();
        let mut member = guild
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test guild.");
        assert!(!member.has_permission(GuildPermission::All, &guild));
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        member.set_permission(GuildPermission::SendMessage, PermissionSetting::FALSE);
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        member.set_permission(GuildPermission::SendMessage, PermissionSetting::NONE);
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        member.set_permission(GuildPermission::SendMessage, PermissionSetting::TRUE);
        assert!(member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        member.set_permission(GuildPermission::All, PermissionSetting::TRUE);
        assert!(member.has_permission(GuildPermission::ReadMessage, &guild));
        assert!(member.has_permission(GuildPermission::SendMessage, &guild));
    }

    #[test]
    fn channel_permissions() {
        let mut guild = get_guild_for_test();
        let mut member = guild
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test guild.");
        assert!(!member.has_permission(GuildPermission::All, &guild));
        let id = ID::from_u128(0);
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &guild));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &guild));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::FALSE,
        );
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &guild));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &guild));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::NONE,
        );
        assert!(!member.has_channel_permission(&id, &ChannelPermission::SendMessage, &guild));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &guild));
        member.set_channel_permission(
            id.clone(),
            ChannelPermission::SendMessage,
            PermissionSetting::TRUE,
        );
        assert!(member.has_channel_permission(&id, &ChannelPermission::SendMessage, &guild));
        assert!(!member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &guild));
        member.set_permission(GuildPermission::All, PermissionSetting::TRUE);
        assert!(member.has_channel_permission(&id, &ChannelPermission::SendMessage, &guild));
        assert!(member.has_channel_permission(&id, &ChannelPermission::ReadMessage, &guild));
    }

    #[test]
    fn rank_permissions() {
        let mut guild = get_guild_for_test();
        let mut member = guild
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test guild.");
        let rank = Rank::new("test_rank".to_string(), ID::from_u128(0));
        guild.ranks.insert(rank.id.clone(), rank.clone());
        member.give_rank(
            guild
                .ranks
                .get_mut(&rank.id)
                .expect("Failed to get test rank."),
        );
        assert!(!member.has_permission(GuildPermission::All, &guild));
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        guild
            .ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(GuildPermission::SendMessage, PermissionSetting::FALSE);
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        guild
            .ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(GuildPermission::SendMessage, PermissionSetting::NONE);
        assert!(!member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        guild
            .ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(GuildPermission::SendMessage, PermissionSetting::TRUE);
        assert!(member.has_permission(GuildPermission::SendMessage, &guild));
        assert!(!member.has_permission(GuildPermission::ReadMessage, &guild));
        guild
            .ranks
            .get_mut(&rank.id)
            .expect("Failed to get test rank.")
            .set_permission(GuildPermission::All, PermissionSetting::TRUE);
        assert!(member.has_permission(GuildPermission::ReadMessage, &guild));
        assert!(member.has_permission(GuildPermission::SendMessage, &guild));
    }

    #[tokio::test]
    async fn channel_view() {
        let mut guild = get_guild_for_test();
        let mut member = guild
            .user_join(&get_user_for_test(2))
            .expect("Test user could not join test guild.");
        {
            {
                let member_in_guild = guild
                    .users
                    .get_mut(&member.user)
                    .expect("Failed to get guild member.");
                member_in_guild
                    .set_permission(GuildPermission::CreateChannel, PermissionSetting::TRUE);
                member = member_in_guild.clone();
            }
            assert!(!member.has_permission(GuildPermission::All, &guild));
            assert!(member.has_permission(GuildPermission::CreateChannel, &guild));
        }
        let channel_0 = guild
            .new_channel(member.user.clone(), "test0".to_string())
            .await
            .expect("Failed to create test channel.");
        let _channel_1 = guild
            .new_channel(member.user.clone(), "test1".to_string())
            .await
            .expect("Failed to create test channel.");
        assert!(guild
            .channels(member.user.clone())
            .expect("Failed to get guild channels.")
            .is_empty());

        {
            let member_in_guild = guild
                .users
                .get_mut(&member.user)
                .expect("Failed to get guild member.");
            member_in_guild.set_channel_permission(
                channel_0.clone(),
                ChannelPermission::ViewChannel,
                PermissionSetting::TRUE,
            );
        }
        let get = guild
            .channels(member.user.clone())
            .expect("Failed to get guild channels.");
        assert_eq!(get.len(), 1);
        assert_eq!(get[0].id, channel_0.clone());
    }

    #[tokio::test]
    async fn save_load() {
        let guild = Guild::new(
            "test".to_string(),
            ID::from_u128(1234),
            &get_user_for_test(1),
        );
        let id_str = guild.id.to_string();
        let id_str = id_str.as_str();
        let _remove = tokio::fs::remove_file(
            "data/guilds/info/".to_string() + &ID::from_u128(1234).to_string() + ".json",
        )
        .await;
        guild.save().await.expect("Failed to save guild info.");
        let load = Guild::load(id_str)
            .await
            .expect("Failed to load guild info.");
        assert_eq!(guild, load);
    }
}
