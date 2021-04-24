use crate::config::Config;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql_warp::Response;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use warp::{http::Response as HttpResponse, Filter};

use crate::error::{Error, Result};
use crate::graphql_model::{MutationRoot, QueryRoot};
use crate::server::Server;
use async_graphql::*;
use xactor::{Actor, Addr};

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let graphql_server_arc = server.clone();

    let graphql_post = warp::any()
        .map(move || graphql_server_arc.clone())
        .and(warp::path("graphql"))
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(
            |server: Arc<Addr<Server>>,
             (schema, request): (
                Schema<QueryRoot, MutationRoot, EmptySubscription>,
                async_graphql::Request,
            )| async move {
                let resp = schema
                    .execute(request.data(server).data("test".to_string()))
                    .await;
                Ok::<_, Infallible>(Response::from(resp))
            },
        );

    let web_socket =
        warp::path!("v2" / "websocket")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let server = server.clone();
                ws.on_upgrade(move |websocket| async move {
                    let _ =
                        crate::websocket::handle_connection(websocket, "test".to_string(), server)
                            .await;
                })
            });

    let graphql_playground = warp::path("graphql_playground").and(warp::get()).map(|| {
        HttpResponse::builder()
            .header("content-type", "text/html")
            .body(playground_source(
                GraphQLPlaygroundConfig::new("/graphql").subscription_endpoint("/"),
            ))
    });

    let routes = graphql_playground.or(graphql_post).or(web_socket);
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
