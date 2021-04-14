use crate::{
    user::{GenericUser, User},
    ID,
};
use async_graphql::*;

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    pub async fn requester<'a>(&self, ctx: &'a Context<'_>) -> &'a ID {
        ctx.data_unchecked::<ID>()
    }

    pub async fn current_user<'a>(&self, ctx: &'a Context<'_>) -> Result<User> {
        Ok(User::load(self.requester(ctx).await?).await.unwrap())
    }

    pub async fn user<'a>(
        &self,
        ctx: &'a Context<'_>,
        #[graphql(desc = "ID of a user.")] id: ID,
    ) -> Result<GenericUser> {
        Ok(User::load(&id)
            .await
            .unwrap()
            .to_generic(self.requester(ctx).await?))
    }

    pub async fn users<'a>(
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
}
