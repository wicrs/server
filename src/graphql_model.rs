use std::{collections::HashSet, sync::Arc};

use crate::{
    channel::Channel,
    hub::{Hub, HubMember, PermissionGroup},
    permission::{ChannelPermission, ChannelPermissionSet, HubPermission, HubPermissionSet},
    server::Server,
    ID,
};
use async_graphql::*;
use chrono::{DateTime, Utc};
use xactor::Addr;

pub type GraphQLSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a ID {
        ctx.data_unchecked::<ID>()
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
        #[graphql(desc = "ID of user hub to check the ban status of.")] id: ID,
    ) -> bool {
        self.bans.contains(&id)
    }

    async fn bans(&self) -> &HashSet<ID> {
        &self.bans
    }

    async fn is_muted(
        &self,
        #[graphql(desc = "ID of the user to check the mute status of.")] id: ID,
    ) -> bool {
        self.mutes.contains(&id)
    }

    async fn mutes(&self) -> &HashSet<ID> {
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
                if ids.contains(id) {
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
        #[graphql(desc = "ID of the hub member to get.")] id: ID,
    ) -> Option<&HubMember> {
        self.members.get(&id)
    }

    async fn members(
        &self,
        #[graphql(desc = "IDs of the members to get.")] ids: Vec<ID>,
    ) -> Vec<&HubMember> {
        self.members
            .iter()
            .filter_map(
                |(id, member)| {
                    if ids.contains(id) {
                        Some(member)
                    } else {
                        None
                    }
                },
            )
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
                    if ids.contains(id) {
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
        #[graphql(desc = "ID of the member to check for the permission.")] id: ID,
        #[graphql(desc = "Permission to check for.")] permission: HubPermission,
    ) -> bool {
        self.members
            .get(&id)
            .map_or(false, |m| m.has_permission(permission, self))
    }

    async fn member_has_channel_permission(
        &self,
        #[graphql(desc = "ID of the member to check for the permission.")] id: ID,
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

    async fn members(&self) -> &Vec<ID> {
        &self.members
    }

    async fn created(&self) -> &DateTime<Utc> {
        &self.created
    }

    async fn is_member(&self, id: ID) -> bool {
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
    async fn user(&self) -> &ID {
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
