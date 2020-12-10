use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::{ApiActionError, ID, JsonLoadError, JsonSaveError, NAME_ALLOWED_CHARS, account_not_found_response, auth::Auth, bad_auth_response, channel::{Channel, Message}, get_system_millis, new_id, permission::{
        ChannelPermission, ChannelPermissions, GuildPermission, GuildPremissions, PermissionSetting,
    }, unexpected_response, user::{Account, User}};

static GUILD_INFO_FOLDER: &str = "data/guilds/info";

#[derive(Serialize, Deserialize, Clone)]
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
        if nickname.chars().all(|c| NAME_ALLOWED_CHARS.contains(c)) {
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

    pub fn has_permission(&mut self, permission: GuildPermission, guild: &Guild) -> bool {
        if self.has_all_permissions() {
            return true;
        }
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
            for rank in self.ranks.iter_mut() {
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
        &mut self,
        channel: &ID,
        permission: &ChannelPermission,
        guild: &Guild,
    ) -> bool {
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
            for rank in self.ranks.iter_mut() {
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

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
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
                GUILD_INFO_FOLDER.to_owned() + "/" + &self.id.to_string(),
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
        if let Ok(json) = tokio::fs::read_to_string(GUILD_INFO_FOLDER.to_owned() + "/" + id).await {
            if let Ok(result) = serde_json::from_str(&json) {
                Ok(result)
            } else {
                Err(JsonLoadError::Deserialize)
            }
        } else {
            Err(JsonLoadError::ReadFile)
        }
    }

    pub fn user_join(&mut self, user: &User) -> Result<(), ()> {
        let mut member = GuildMember::new(user, self.id.clone());
        for (id, rank) in self.ranks.iter_mut() {
            if id == &self.default_rank {
                rank.add_member(&mut member);
                break;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct GuildCreateQuery {
    id: String,
    user: ID,
    token: String,
    name: String,
}

fn api_v1_create(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path("create"))
        .and(warp::body::json::<GuildCreateQuery>())
        .and_then(move |query: GuildCreateQuery| {
            let tmp_auth = auth_manager.clone();
            async move {
                Ok::<_, warp::Rejection>(
                    if Auth::is_authenticated(tmp_auth, &query.id, query.token).await {
                        if let Ok(mut account) = Account::load(&query.id).await {
                            let create = account.create_guild(query.name, query.user).await;
                            if let Err(err) = create {
                                match err {
                                    ApiActionError::OpenFileError
                                    | ApiActionError::WriteFileError => warp::reply::with_status(
                                        "Server could not save the guild data.",
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                    )
                                    .into_response(),

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
                                warp::reply::with_status(ok.to_string(), StatusCode::OK)
                                    .into_response()
                            } else {
                                unexpected_response()
                            }
                        } else {
                            account_not_found_response()
                        }
                    } else {
                        bad_auth_response()
                    },
                )
            }
        })
        .boxed()
}

#[derive(Deserialize, Serialize)]
struct MessageSendQuery {
    id: String,
    token: String,
    user: ID,
    guild: ID,
    channel: ID,
    message: String,
}

fn api_v1_sendmessage(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    warp::get()
        .and(warp::path("send_message"))
        .and(warp::body::json::<MessageSendQuery>())
        .and_then(move |query: MessageSendQuery| {
            let tmp_auth = auth_manager.clone();
            async move {
                Ok::<_, warp::Rejection>(if Auth::is_authenticated(tmp_auth, &query.id, query.token).await {
                    if let Ok(account) = Account::load(&query.id).await {
                        if let Err(err) = account.send_guild_message(query.user, query.guild, query.channel, query.message).await {
                            match err {
                                ApiActionError::GuildNotFound | ApiActionError::NotInGuild => warp::reply::with_status("You are not in that guild if it exists.", StatusCode::NOT_FOUND).into_response(),
                                ApiActionError::ChannelNotFound | ApiActionError::NoPermission => warp::reply::with_status("You do not have permission to access that channel if it exists.", StatusCode::NOT_FOUND).into_response(),
                                ApiActionError::OpenFileError | ApiActionError::WriteFileError => warp::reply::with_status("Server could not save your message.", StatusCode::INTERNAL_SERVER_ERROR).into_response(),
                                ApiActionError::UserNotFound => warp::reply::with_status("That user does not exist on your account.", StatusCode::INTERNAL_SERVER_ERROR).into_response(),
                                _ => unexpected_response()
                            }
                        } else {
                            warp::reply::with_status("Message sent successfully.", StatusCode::OK).into_response()
                        }
                    } else {
                        account_not_found_response()
                    }
                } else {
                    bad_auth_response()
                })
            }
        }).boxed()
}

#[derive(Deserialize, Serialize)]
struct ChannelsQuery {
    account: String,
    token: String,
    user: ID,
    guild: ID
}

pub fn api_v1(auth_manager: Arc<Mutex<Auth>>) -> BoxedFilter<(impl Reply,)> {
    api_v1_create(auth_manager.clone())
        .or(api_v1_sendmessage(auth_manager))
        .boxed()
}
