use std::collections::HashMap;

use crate::{
    channel::Channel,
    hub::{Hub, HubMember, PermissionGroup},
    permission::{ChannelPermissionSet, HubPermissionSet},
    user::{GenericUser, User},
    ID,
};
use async_graphql::*;

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

#[ComplexObject]
impl Hub {
    async fn channels(&self) -> Vec<Channel> {
        self.channels
            .iter()
            .map(|(_, channel)| channel.clone())
            .collect()
    }
    async fn members(&self) -> Vec<HubMember> {
        self.members
            .iter()
            .map(|(_, member)| member.clone())
            .collect()
    }
    async fn groups(&self) -> Vec<PermissionGroup> {
        self.groups.iter().map(|(_, group)| group.clone()).collect()
    }
}

#[ComplexObject]
impl PermissionGroup {
    async fn hub_permissions(&self) -> HashMap<String, bool> {
        self.hub_permissions
            .iter()
            .filter_map(|(permission, setting)| {
                if let Some(setting) = setting {
                    Some((permission.to_string(), setting.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    async fn channel_permissions(&self) -> HashMap<String, HashMap<String, bool>> {
        self.channel_permissions
            .iter()
            .map(|(channel, permissions)| {
                (
                    channel.to_string(),
                    permissions
                        .iter()
                        .filter_map(|(permission, setting)| {
                            if let Some(setting) = setting {
                                Some((permission.to_string(), setting.clone()))
                            } else {
                                None
                            }
                        })
                        .collect(),
                )
            })
            .collect()
    }
}

#[ComplexObject]
impl HubMember {
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
