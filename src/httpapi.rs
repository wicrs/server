use crate::{auth::Auth, config::Config};
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::Response;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::{http::Response as HttpResponse, Filter};

use crate::error::{Error, Result};
use crate::graphql_model::{QueryRoot, MutationRoot};
use crate::server::Server;
use crate::ID;
use async_graphql::Response as AsyncGraphQLResponse;
use async_graphql::*;
use xactor::{Actor, Addr};

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let (auth, test_id, test_token) = Auth::for_testing(20).await;
    let auth = Arc::new(RwLock::new(auth));
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let graphql_auth_arc = auth.clone();
    let graphql_server_arc = server.clone();

    println!("Playground: http://{}", config.address);

    let graphql_post = warp::any()
        .map(move || (graphql_auth_arc.clone(), graphql_server_arc.clone()))
        .and(warp::path!("graphql" / String))
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            |(auth, server): (Arc<RwLock<Auth>>, Arc<Addr<Server>>),
             token: String,
             (schema, request): (
                Schema<QueryRoot, MutationRoot, EmptySubscription>,
                async_graphql::Request,
            )| async move {
                let mut split = token.as_str().split(':');
                if let (Some(id), Some(token)) = (split.next(), split.next()) {
                    if let Ok(id) = ID::parse_str(id) {
                        if Auth::is_authenticated(auth.clone(), id.clone(), token.into()).await {
                            let resp = schema.execute(request.data(id).data(server)).await;
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
    warp::serve(routes)
        .run(
            config
                .address
                .parse::<SocketAddr>()
                .expect("Unable to parse server bind address."),
        )
        .await;
    Ok(())
}
