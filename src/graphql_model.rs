use std::{collections::HashSet, sync::Arc};

use crate::{
    api,
    channel::{Channel, Message},
    hub::{Hub, HubMember, PermissionGroup},
    permission::{ChannelPermission, ChannelPermissionSet, HubPermission, HubPermissionSet},
    server::Server,
    user::{GenericUser, User},
    ID,
};
use async_graphql::*;
use chrono::{DateTime, Utc};
use xactor::Addr;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a ID {
        ctx.data_unchecked::<ID>()
    }

    async fn current_user(&self, ctx: &Context<'_>) -> Result<User> {
        Ok(User::load(self.requester(ctx).await?).await.unwrap())
    }

    async fn user(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID of a user.")] id: ID,
    ) -> Result<GenericUser> {
        Ok(User::load(&id)
            .await
            .unwrap()
            .to_generic(self.requester(ctx).await?))
    }

    async fn users(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "List of the IDs of the users to get.")] ids: Vec<ID>,
    ) -> Result<Vec<GenericUser>> {
        let mut result = Vec::new();
        for id in ids {
            result.push(
                User::load(&id)
                    .await
                    .unwrap()
                    .to_generic(self.requester(ctx).await?),
            );
        }
        Ok(result)
    }

    async fn hub(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID of a user.")] id: ID,
    ) -> Result<Hub> {
        Ok(Hub::load(&id)
            .await
            .unwrap()
            .strip(self.requester(ctx).await?)
            .unwrap())
    }
}

pub struct MutationRoot;

struct UserMutator {
    user_id: ID,
}

impl UserMutator {
    fn new(user_id: ID) -> Self {
        Self { user_id }
    }
}

#[Object]
impl UserMutator {
    async fn username(&self, new_name: String) -> Result<String> {
        Ok(api::change_username(&self.user_id, new_name).await?)
    }
    async fn status(&self, new_status: String) -> Result<String> {
        Ok(api::change_user_status(&self.user_id, new_status).await?)
    }
    async fn description(&self, new_description: String) -> Result<String> {
        Ok(api::change_user_description(&self.user_id, new_description).await?)
    }
    async fn join_hub(&self, hub_id: ID) -> Result<ID> {
        Ok(api::join_hub(&self.user_id, &hub_id)
            .await
            .and(Ok(hub_id))?)
    }
    async fn leave_hub(&self, hub_id: ID) -> Result<ID> {
        Ok(api::leave_hub(&self.user_id, &hub_id)
            .await
            .and(Ok(hub_id))?)
    }
}

struct HubMutator {
    user_id: ID,
    hub_id: ID,
}

impl HubMutator {
    fn new(user_id: ID, hub_id: ID) -> Self {
        Self { user_id, hub_id }
    }
}

#[Object]
impl HubMutator {
    async fn name(&self, new_name: String) -> Result<String> {
        Ok(api::rename_hub(&self.user_id, &self.hub_id, new_name).await?)
    }
    async fn description(&self, new_description: String) -> Result<String> {
        Ok(api::change_hub_description(&self.user_id, &self.hub_id, new_description).await?)
    }
    async fn ban(&self, user_id: ID) -> Result<ID> {
        Ok(api::ban_user(&self.user_id, &self.hub_id, &user_id).await.and(Ok(user_id))?)
    }
    async fn unban(&self, user_id: ID) -> Result<ID> {
        Ok(api::unban_user(&self.user_id, &self.hub_id, &user_id).await.and(Ok(user_id))?)
    }
    async fn mute(&self, user_id: ID) -> Result<ID> {
        Ok(api::ban_user(&self.user_id, &self.hub_id, &user_id).await.and(Ok(user_id))?)
    }
    async fn unmute(&self, user_id: ID) -> Result<ID> {
        Ok(api::unmute_user(&self.user_id, &self.hub_id, &user_id).await.and(Ok(user_id))?)
    }
}

#[Object]
impl MutationRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a ID {
        ctx.data_unchecked::<ID>()
    }

    async fn user(&self, ctx: &Context<'_>) -> Result<UserMutator> {
        Ok(UserMutator::new(*self.requester(ctx).await?))
    }

    async fn hub(&self, ctx: &Context<'_>, hub_id: ID) -> Result<HubMutator> {
        Ok(HubMutator::new(*self.requester(ctx).await?, hub_id))
    }
}

#[Object]
impl Channel {
    async fn id(&self) -> &ID {
        &self.id
    }

    async fn name(&self) -> &String {
        &self.name
    }

    async fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    async fn description(&self) -> &String {
        &self.description
    }

    async fn message(&self, id: ID) -> Option<Message> {
        self.get_message(&id).await
    }

    async fn messages(&self, ids: Vec<ID>) -> Vec<Message> {
        self.get_messages(ids).await
    }

    async fn search_messages(&self, ctx: &Context<'_>, query: String, limit: u8) -> Vec<ID> {
        if let Ok(ms_addr) = ctx
            .data_unchecked::<Arc<Addr<Server>>>()
            .call(crate::server::GetMessageServer)
            .await
        {
            ms_addr
                .call(crate::server::SearchMessageIndex {
                    hub_id: self.hub_id.clone(),
                    channel_id: self.id.clone(),
                    limit: limit as usize,
                    query: query,
                })
                .await
                .map_or(Vec::new(), |r| r.unwrap_or_default())
        } else {
            Vec::new()
        }
    }

    async fn messages_after(&self, id: ID, max: u8) -> Vec<Message> {
        self.get_messages_after(&id, max as usize).await
    }

    async fn messages_between(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        invert: bool,
        max: u8,
    ) -> Vec<Message> {
        self.get_messages_between(from, to, invert, max as usize)
            .await
    }

    async fn messages_containing(
        &self,
        max: u8,
        string: String,
        case_sensitive: bool,
    ) -> Vec<Message> {
        self.find_messages_containing(string, case_sensitive, max as usize)
            .await
    }
}

#[Object]
impl Hub {
    async fn id(&self) -> &ID {
        &self.id
    }

    async fn name(&self) -> &String {
        &self.name
    }

    async fn owner(&self) -> Option<&HubMember> {
        self.members.get(&self.owner)
    }

    async fn default_group(&self) -> Option<&PermissionGroup> {
        self.groups.get(&self.default_group)
    }

    async fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    async fn description(&self) -> &String {
        &self.description
    }

    async fn is_banned(&self, id: ID) -> bool {
        self.bans.contains(&id)
    }

    async fn bans(&self) -> &HashSet<ID> {
        &self.bans
    }

    async fn is_muted(&self, id: ID) -> bool {
        self.mutes.contains(&id)
    }

    async fn mutes(&self) -> &HashSet<ID> {
        &self.mutes
    }

    async fn channel(&self, id: ID) -> Option<&Channel> {
        self.channels.get(&id)
    }

    async fn channels(&self) -> Vec<&Channel> {
        self.channels.iter().map(|(_, channel)| channel).collect()
    }

    async fn member(&self, id: ID) -> Option<&HubMember> {
        self.members.get(&id)
    }

    async fn members(&self) -> Vec<&HubMember> {
        self.members.iter().map(|(_, member)| member).collect()
    }

    async fn group(&self, id: ID) -> Option<&PermissionGroup> {
        self.groups.get(&id)
    }

    async fn groups(&self) -> Vec<&PermissionGroup> {
        self.groups.iter().map(|(_, group)| group).collect()
    }

    async fn member_has_permission(&self, id: ID, permission: HubPermission) -> bool {
        self.members
            .get(&id)
            .map_or(false, |m| m.has_permission(permission, self))
    }

    async fn member_has_channel_permission(
        &self,
        id: ID,
        channel: ID,
        permission: ChannelPermission,
    ) -> bool {
        self.members.get(&id).map_or(false, |m| {
            m.has_channel_permission(&channel, permission, self)
        })
    }
}

#[Object]
impl PermissionGroup {
    async fn id(&self) -> &ID {
        &self.id
    }

    async fn name(&self) -> &String {
        &self.name
    }

    async fn members(&self) -> &Vec<ID> {
        &self.members
    }

    async fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    async fn is_member(&self, id: ID) -> bool {
        self.members.contains(&id)
    }

    async fn hub_permission(&self, permission: HubPermission) -> Option<HubPermissionSet> {
        if let Some(setting) = self.hub_permissions.get(&permission) {
            Some(HubPermissionSet {
                permission,
                setting: setting.clone(),
            })
        } else {
            None
        }
    }

    async fn hub_permissions(&self) -> Vec<HubPermissionSet> {
        self.hub_permissions
            .iter()
            .filter_map(|(permission, setting)| {
                if let Some(setting) = setting {
                    Some(HubPermissionSet::from((
                        permission.clone(),
                        Some(setting.clone()),
                    )))
                } else {
                    None
                }
            })
            .collect()
    }

    async fn channel_permission(
        &self,
        channel: ID,
        permission: ChannelPermission,
    ) -> Option<ChannelPermissionSet> {
        if let Some(setting) = self.channel_permissions.get(&channel) {
            setting.get(&permission).map_or(None, |s| {
                Some(ChannelPermissionSet {
                    permission,
                    setting: s.clone(),
                    channel,
                })
            })
        } else {
            None
        }
    }

    async fn channel_permissions(&self) -> Vec<ChannelPermissionSet> {
        let mut result = Vec::new();
        self.channel_permissions
            .iter()
            .for_each(|(channel, permissions)| {
                result.append(
                    &mut permissions
                        .iter()
                        .filter_map(|(permission, setting)| {
                            if let Some(setting) = setting {
                                Some(ChannelPermissionSet::from((
                                    permission.clone(),
                                    Some(setting.clone()),
                                    channel.clone(),
                                )))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<ChannelPermissionSet>>(),
                )
            });
        result
    }
}

#[Object]
impl HubMember {
    async fn user(&self) -> &ID {
        &self.user
    }

    async fn nickname(&self) -> &String {
        &self.nickname
    }

    async fn groups(&self) -> &Vec<ID> {
        &self.groups
    }

    async fn in_group(&self, id: ID) -> bool {
        self.groups.contains(&id)
    }

    async fn joined(&self) -> &DateTime<Utc> {
        &self.joined
    }

    async fn hub_permission(&self, permission: HubPermission) -> Option<HubPermissionSet> {
        if let Some(setting) = self.hub_permissions.get(&permission) {
            Some(HubPermissionSet {
                permission,
                setting: setting.clone(),
            })
        } else {
            None
        }
    }

    async fn hub_permissions(&self) -> Vec<HubPermissionSet> {
        self.hub_permissions
            .iter()
            .filter_map(|(permission, setting)| {
                if let Some(setting) = setting {
                    Some(HubPermissionSet::from((
                        permission.clone(),
                        Some(setting.clone()),
                    )))
                } else {
                    None
                }
            })
            .collect()
    }

    async fn channel_permission(
        &self,
        channel: ID,
        permission: ChannelPermission,
    ) -> Option<ChannelPermissionSet> {
        if let Some(setting) = self.channel_permissions.get(&channel) {
            setting.get(&permission).map_or(None, |s| {
                Some(ChannelPermissionSet {
                    permission,
                    setting: s.clone(),
                    channel,
                })
            })
        } else {
            None
        }
    }

    async fn channel_permissions(&self) -> Vec<ChannelPermissionSet> {
        let mut result = Vec::new();
        self.channel_permissions
            .iter()
            .for_each(|(channel, permissions)| {
                result.append(
                    &mut permissions
                        .iter()
                        .filter_map(|(permission, setting)| {
                            if let Some(setting) = setting {
                                Some(ChannelPermissionSet::from((
                                    permission.clone(),
                                    Some(setting.clone()),
                                    channel.clone(),
                                )))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<ChannelPermissionSet>>(),
                )
            });
        result
    }
}
