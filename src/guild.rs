use std::{
    collections::{HashMap, HashSet},
    vec,
};

use crate::{ID, channel::Channel, get_system_millis, new_id, permission::{
        ChannelPermission, ChannelPermissions, GuildPermission, GuildPremissions, PermissionSetting,
    }, user::User};

pub struct GuildMember {
    pub user: User,
    pub joined: u128,
    pub nickname: String,
    pub ranks: Vec<ID>,
    pub guild_permissions: GuildPremissions,
    pub channel_permissions: HashMap<ID, ChannelPermissions>,
}

impl GuildMember {
    pub fn new(user: User) -> Self {
        Self {
            nickname: user.username.clone(),
            user,
            ranks: Vec::new(),
            joined: get_system_millis(),
            guild_permissions: HashMap::new(),
            channel_permissions: HashMap::new(),
        }
    }

    pub fn give_rank(&mut self, rank: &mut Rank) {
        if !self.ranks.contains(&rank.id) {
            self.ranks.push(rank.id.clone());
        }
        if !rank.members.contains(&self.user.id) {
            rank.members.push(self.user.id.clone());
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
        } false
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
                    PermissionSetting::NONE => {}
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
        } false
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
                }
            }
        }
        return false;
    }
}

pub struct Guild {
    pub channels: Vec<Channel>,
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
    pub fn new(name: String, id: ID, creator: User) -> Self {
        let creator_id = creator.id.clone();
        let mut everyone = Rank::new(String::from("everyone"), new_id());
        let mut owner = GuildMember::new(creator);
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
            channels: vec![],
            users,
            created: get_system_millis(),
        }
    }

    pub fn user_join(&mut self, user: User) -> Result<(), ()> {
        let mut member = GuildMember::new(user);
        for (id, rank) in self.ranks.iter_mut() {
            if id == &self.default_rank {
                rank.add_member(&mut member);
                break;
            }
        }
        Ok(())
    }
}
