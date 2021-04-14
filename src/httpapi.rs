use crate::{auth::Auth, config::Config};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::Response;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{http::Response as HttpResponse, Filter};

use crate::ID;
use async_graphql::Response as AsyncGraphQLResponse;
use async_graphql::*;

pub async fn graphql(_config: Config) {
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();
    let (auth, test_id, test_token) = Auth::for_testing().await;
    let auth = Arc::new(RwLock::new(auth));

    println!("Playground: http://localhost:8000");

    let graphql_post = warp::any()
        .map(move || auth.clone())
        .and(warp::path!("graphql" / String))
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            |auth: Arc<RwLock<Auth>>,
             token: String,
             (schema, mut request): (
                Schema<Query, EmptyMutation, EmptySubscription>,
                async_graphql::Request,
            )| async move {
                let mut split = token.as_str().split(':');
                if let (Some(id), Some(token)) = (split.next(), split.next()) {
                    if let Ok(id) = ID::parse_str(id) {
                        if Auth::is_authenticated(auth.clone(), id.clone(), token.into()).await {
                            request = request.data(id);
                            let resp = schema.execute(request).await;
                            return Ok::<_, Infallible>(Response::from(resp));
                        }
                    }
                }
                Ok::<_, Infallible>(Response::from(AsyncGraphQLResponse::new(
                    "Not Authenticated.",
                )))
            },
        );

    let graphql_playground = warp::path::end().and(warp::get()).map(move || {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new(
                    format!("/graphql/{}:{}", test_id, test_token).as_str(),
                )
                .subscription_endpoint("/"),
            ))
    });

    let routes = graphql_playground.or(graphql_post);
    warp::serve(routes).run(([0, 0, 0, 0], 8000)).await;
}

struct Query;

#[Object]
impl Query {
    async fn requester<'a>(&self, ctx: &'a Context<'_>) -> async_graphql::Result<&'a ID> {
        ctx.data::<ID>()
    }

    async fn user(&self, id: ID) -> String {
        id.to_string()
    }
}
