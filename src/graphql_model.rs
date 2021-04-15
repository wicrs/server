use std::collections::HashSet;

use crate::{
    channel::Channel,
    hub::{Hub, HubMember, PermissionGroup},
    permission::{ChannelPermission, ChannelPermissionSet, HubPermission, HubPermissionSet},
    user::{GenericUser, User},
    ID,
};
use async_graphql::*;
use chrono::{DateTime, Utc};

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a ID {
        ctx.data_unchecked::<ID>()
    }

    async fn current_user<'a>(&self, ctx: &'a Context<'_>) -> Result<User> {
        Ok(User::load(self.requester(ctx).await?).await.unwrap())
    }

    async fn user<'a>(
        &self,
        ctx: &'a Context<'_>,
        #[graphql(desc = "ID of a user.")] id: ID,
    ) -> Result<GenericUser> {
        Ok(User::load(&id)
            .await
            .unwrap()
            .to_generic(self.requester(ctx).await?))
    }

    async fn users<'a>(
        &self,
        ctx: &'a Context<'_>,
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

    async fn hub<'a>(
        &self,
        ctx: &'a Context<'_>,
        #[graphql(desc = "ID of a user.")] id: ID,
    ) -> Result<Hub> {
        Ok(Hub::load(&id)
            .await
            .unwrap()
            .strip(self.requester(ctx).await?)
            .unwrap())
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
