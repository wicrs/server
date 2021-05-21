use std::{collections::HashSet, sync::Arc};

use crate::{
    api,
    channel::Channel,
    hub::{Hub, HubMember, PermissionGroup},
    permission::{ChannelPermission, ChannelPermissionSet, HubPermission, HubPermissionSet},
    server::Server,
    ID,
};
use async_graphql::*;
use chrono::{DateTime, Utc};
use xactor::Addr;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a String {
        ctx.data_unchecked::<String>()
    }

    async fn hub(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID of a hub.")] id: ID,
    ) -> Result<Hub> {
        let hub = Hub::load(id).await?;
        Ok(hub.strip(self.requester(ctx).await?)?)
    }

    async fn hubs(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "List of the IDs of the hubs to get.")] ids: Vec<ID>,
    ) -> Result<Vec<Hub>> {
        let mut result = Vec::new();
        for id in ids {
            let hub = Hub::load(id).await?;
            result.push(hub.strip(self.requester(ctx).await?)?);
        }
        Ok(result)
    }
}

pub struct MutationRoot;

struct ChannelMutator {
    user_id: String,
    hub_id: ID,
    channel_id: ID,
}

impl ChannelMutator {
    fn new(user_id: String, hub_id: ID, channel_id: ID) -> Self {
        Self {
            user_id,
            hub_id,
            channel_id,
        }
    }
}

#[Object]
impl ChannelMutator {
    async fn name(
        &self,
        #[graphql(desc = "New name for the channel.")] new: String,
    ) -> Result<String> {
        Ok(api::rename_channel(&self.user_id, self.hub_id, self.channel_id, new).await?)
    }
    async fn description(
        &self,
        #[graphql(desc = "New description for the channel.")] new: String,
    ) -> Result<String> {
        Ok(
            api::change_channel_description(&self.user_id, self.hub_id, self.channel_id, new)
                .await?,
        )
    }
}

struct HubMutator {
    user_id: String,
    hub_id: ID,
}

impl HubMutator {
    fn new(user_id: String, hub_id: ID) -> Self {
        Self { user_id, hub_id }
    }
}

#[Object]
impl HubMutator {
    async fn name(&self, #[graphql(desc = "New name for the hub.")] new: String) -> Result<String> {
        Ok(api::rename_hub(&self.user_id, self.hub_id, new).await?)
    }
    async fn description(
        &self,
        #[graphql(desc = "New description for the hub.")] new: String,
    ) -> Result<String> {
        Ok(api::change_hub_description(&self.user_id, self.hub_id, new).await?)
    }
    async fn channel(
        &self,
        #[graphql(desc = "ID of the channel to get.")] id: ID,
    ) -> ChannelMutator {
        ChannelMutator::new(self.user_id.clone(), self.hub_id, id)
    }
    async fn delete_channel(
        &self,
        #[graphql(desc = "ID of the channel to delete.")] id: ID,
    ) -> Result<ID> {
        Ok(api::delete_channel(&self.user_id, self.hub_id, id)
            .await
            .and(Ok(id))?)
    }
    async fn create_channel(
        &self,
        #[graphql(desc = "Name for the new channel.")] name: String,
    ) -> Result<Channel> {
        Ok(api::get_channel(
            &self.user_id,
            self.hub_id,
            api::create_channel(&self.user_id, self.hub_id, name).await?,
        )
        .await?)
    }
    async fn kick(
        &self,
        #[graphql(desc = "ID of the user to kick.")] id: String,
    ) -> Result<String> {
        Ok(api::kick_user(&self.user_id, self.hub_id, &id)
            .await
            .and(Ok(id))?)
    }
    async fn ban(&self, #[graphql(desc = "ID of the user to ban.")] id: String) -> Result<String> {
        Ok(api::ban_user(&self.user_id, self.hub_id, &id)
            .await
            .and(Ok(id))?)
    }
    async fn unban(
        &self,
        #[graphql(desc = "ID of the user to unban.")] id: String,
    ) -> Result<String> {
        Ok(api::unban_user(&self.user_id, self.hub_id, &id)
            .await
            .and(Ok(id))?)
    }
    async fn mute(
        &self,
        #[graphql(desc = "ID of the user to mute.")] id: String,
    ) -> Result<String> {
        Ok(api::ban_user(&self.user_id, self.hub_id, &id)
            .await
            .and(Ok(id))?)
    }
    async fn unmute(
        &self,
        #[graphql(desc = "ID of the user to unmute.")] id: String,
    ) -> Result<String> {
        Ok(api::unmute_user(&self.user_id, self.hub_id, &id)
            .await
            .and(Ok(id))?)
    }
}

#[Object]
impl MutationRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a String {
        ctx.data_unchecked::<String>()
    }

    async fn hub(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID of the hub to get.")] id: ID,
    ) -> Result<HubMutator> {
        Ok(HubMutator::new(self.requester(ctx).await?.clone(), id))
    }

    async fn delete_hub(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "ID of the hub to delete.")] id: ID,
    ) -> Result<ID> {
        Ok(api::delete_hub(self.requester(ctx).await?, id)
            .await
            .and(Ok(id))?)
    }

    async fn create_hub(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Name for the new hub.")] name: String,
    ) -> Result<Hub> {
        Ok(Hub::load(api::create_hub(self.requester(ctx).await?.clone(), name).await?).await?)
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

    async fn search_messages(
        &self,
        ctx: &Context<'_>,
        #[graphql(desc = "Query that messages should match.")] query: String,
        #[graphql(desc = "Maximum number of messages to get.")] limit: u8,
    ) -> Vec<ID> {
        if let Ok(ms_addr) = ctx
            .data_unchecked::<Arc<Addr<Server>>>()
            .call(crate::server::GetMessageServer)
            .await
        {
            ms_addr
                .call(crate::server::SearchMessageIndex {
                    hub_id: self.hub_id,
                    channel_id: self.id,
                    limit: limit as usize,
                    query,
                })
                .await
                .map_or(Vec::new(), |r| r.unwrap_or_default())
        } else {
            Vec::new()
        }
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

    async fn is_banned(
        &self,
        #[graphql(desc = "ID of user hub to check the ban status of.")] id: String,
    ) -> bool {
        self.bans.contains(&id)
    }

    async fn bans(&self) -> &HashSet<String> {
        &self.bans
    }

    async fn is_muted(
        &self,
        #[graphql(desc = "ID of the user to check the mute status of.")] id: String,
    ) -> bool {
        self.mutes.contains(&id)
    }

    async fn mutes(&self) -> &HashSet<String> {
        &self.mutes
    }

    async fn channel(
        &self,
        #[graphql(desc = "ID of the channel to get.")] id: ID,
    ) -> Option<&Channel> {
        self.channels.get(&id)
    }

    async fn channels(
        &self,
        #[graphql(desc = "IDs of the channels to get.")] ids: Vec<ID>,
    ) -> Vec<&Channel> {
        self.channels
            .iter()
            .filter_map(|(id, channel)| {
                if ids.contains(&id) {
                    Some(channel)
                } else {
                    None
                }
            })
            .collect()
    }

    async fn all_channels(&self) -> Vec<&Channel> {
        self.channels.iter().map(|(_, channel)| channel).collect()
    }

    async fn member(
        &self,
        #[graphql(desc = "ID of the hub member to get.")] id: String,
    ) -> Option<&HubMember> {
        self.members.get(&id)
    }

    async fn members(
        &self,
        #[graphql(desc = "IDs of the members to get.")] ids: Vec<String>,
    ) -> Vec<&HubMember> {
        self.members
            .iter()
            .filter_map(|(id, member)| {
                if ids.contains(&id) {
                    Some(member)
                } else {
                    None
                }
            })
            .collect()
    }

    async fn all_members(&self) -> Vec<&HubMember> {
        self.members.iter().map(|(_, member)| member).collect()
    }

    async fn group(
        &self,
        #[graphql(desc = "ID of the permission group to get.")] id: ID,
    ) -> Option<&PermissionGroup> {
        self.groups.get(&id)
    }

    async fn groups(
        &self,
        #[graphql(desc = "IDs of the permission groups to get.")] ids: Vec<ID>,
    ) -> Vec<&PermissionGroup> {
        self.groups
            .iter()
            .filter_map(
                |(id, group)| {
                    if ids.contains(&id) {
                        Some(group)
                    } else {
                        None
                    }
                },
            )
            .collect()
    }

    async fn all_groups(&self) -> Vec<&PermissionGroup> {
        self.groups.iter().map(|(_, group)| group).collect()
    }

    async fn member_has_permission(
        &self,
        #[graphql(desc = "ID of the member to check for the permission.")] id: String,
        #[graphql(desc = "Permission to check for.")] permission: HubPermission,
    ) -> bool {
        self.members
            .get(&id)
            .map_or(false, |m| m.has_permission(permission, self))
    }

    async fn member_has_channel_permission(
        &self,
        #[graphql(desc = "ID of the member to check for the permission.")] id: String,
        #[graphql(
            desc = "ID of the channel to check in which to check the setting of the permission."
        )]
        channel: ID,
        #[graphql(desc = "Permission to check for.")] permission: ChannelPermission,
    ) -> bool {
        self.members.get(&id).map_or(false, |m| {
            m.has_channel_permission(channel, permission, self)
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

    async fn members(&self) -> &Vec<String> {
        &self.members
    }

    async fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    async fn is_member(&self, id: String) -> bool {
        self.members.contains(&id)
    }

    async fn hub_permission(
        &self,
        #[graphql(desc = "Permission to check for.")] permission: HubPermission,
    ) -> Option<HubPermissionSet> {
        self.hub_permissions
            .get(&permission)
            .map(|setting| HubPermissionSet {
                permission,
                setting: *setting,
            })
    }

    async fn hub_permissions(&self) -> Vec<HubPermissionSet> {
        self.hub_permissions
            .iter()
            .filter_map(|(permission, setting)| {
                setting
                    .as_ref()
                    .map(|setting| HubPermissionSet::from((*permission, Some(*setting))))
            })
            .collect()
    }

    async fn channel_permission(
        &self,
        #[graphql(desc = "Channel in which to check for the permission.")] channel: ID,
        #[graphql(desc = "Permission to check for.")] permission: ChannelPermission,
    ) -> Option<ChannelPermissionSet> {
        if let Some(setting) = self.channel_permissions.get(&channel) {
            setting.get(&permission).map(|s| ChannelPermissionSet {
                permission,
                setting: *s,
                channel,
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
                            setting.as_ref().map(|setting| {
                                ChannelPermissionSet::from((*permission, Some(*setting), *channel))
                            })
                        })
                        .collect::<Vec<ChannelPermissionSet>>(),
                )
            });
        result
    }
}

#[Object]
impl HubMember {
    async fn user(&self) -> &String {
        &self.user_id
    }

    async fn groups(&self) -> &Vec<ID> {
        &self.groups
    }

    async fn in_group(
        &self,
        #[graphql(desc = "ID of the permission group to check for membership.")] id: ID,
    ) -> bool {
        self.groups.contains(&id)
    }

    async fn joined(&self) -> &DateTime<Utc> {
        &self.joined
    }

    async fn hub_permission(
        &self,
        #[graphql(desc = "Permission to check for.")] permission: HubPermission,
    ) -> Option<HubPermissionSet> {
        self.hub_permissions
            .get(&permission)
            .map(|setting| HubPermissionSet {
                permission,
                setting: *setting,
            })
    }

    async fn hub_permissions(&self) -> Vec<HubPermissionSet> {
        self.hub_permissions
            .iter()
            .filter_map(|(permission, setting)| {
                setting
                    .as_ref()
                    .map(|setting| HubPermissionSet::from((*permission, Some(*setting))))
            })
            .collect()
    }

    async fn channel_permission(
        &self,
        #[graphql(desc = "Permission to check for.")] permission: ChannelPermission,
        channel: ID,
    ) -> Option<ChannelPermissionSet> {
        if let Some(setting) = self.channel_permissions.get(&channel) {
            setting.get(&permission).map(|s| ChannelPermissionSet {
                permission,
                setting: *s,
                channel,
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
                            setting.as_ref().map(|setting| {
                                ChannelPermissionSet::from((*permission, Some(*setting), *channel))
                            })
                        })
                        .collect::<Vec<ChannelPermissionSet>>(),
                )
            });
        result
    }
}
