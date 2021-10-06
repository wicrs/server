use async_graphql::extensions::ApolloTracing;
use async_graphql::{EmptyMutation, EmptySubscription, Schema};

use serde::{Deserialize, Serialize};
use xactor::Actor;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::graphql_model::GraphQLSchema;
use crate::server::Server;
use crate::ID;
use crate::{api, graphql_model::QueryRoot, server::ServerAddress};
use warp::Reply;
use warp::{Filter, Rejection};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
}

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .extension(ApolloTracing)
        .finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );

    /*let schema_sdl = schema.sdl();
    let graphql_schema =
        warp::path("graphql_schema").map(move || schema_sdl.clone().into_response());

    let server_info_struct = ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    let server_info_str = serde_json::to_string(&server_info_struct).unwrap();

    let server_info =
        warp::path!("v3" / "info").map(move || server_info_str.clone().into_response());

    let cors = warp::cors()
        .allow_header("content-type")
        .allow_header("authorization")
        .allow_header("cache-control")
        .allow_any_origin();
    let log = warp::log("wicrs_server::httpapi");

    let routes = warp::path("v3")
    .and(api(Arc::clone(&server), schema))
    .with(cors)
    .with(log);*/

    let server = warp::serve(api(server, schema)).run(
        config
            .address
            .parse::<SocketAddr>()
            .expect("Invalid bind address"),
    );

    server.await;

    Ok(())
}

fn auth() -> impl Filter<Extract = (ID,), Error = warp::Rejection> + Clone {
    warp::header("authorization")
}

fn with_server(
    server: ServerAddress,
) -> impl Filter<Extract = (ServerAddress,), Error = Infallible> + Clone {
    warp::any().map(move || Arc::clone(&server))
}

fn websocket(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("websocket")
        .and(with_server(server))
        .and(auth())
        .and(warp::ws())
        .and_then(api::websocket)
}

fn graphql(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("graphql")
        .and(with_server(server))
        .and(auth())
        .and(async_graphql_warp::graphql(schema.clone()))
        .and_then(api::graphql)
}

fn send_message(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::post()
        .and(with_server(server))
        .and(auth())
        .and(warp::path!(ID / ID))
        .and(warp::body::json())
        .and_then(api::send_message)
}

fn create_hub() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::post()
        .and(auth())
        .and(warp::path!(String))
        .and_then(api::create_hub)
}

fn update_hub(
    server: ServerAddress,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    with_server(server)
        .and(warp::put())
        .and(auth())
        .and(warp::path!(ID))
        .and(warp::body::json())
        .and_then(api::update_hub)
}

fn get_hub() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::get()
        .and(auth())
        .and(warp::path!(ID))
        .and_then(api::get_hub)
}

fn rest(server: ServerAddress) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let hub = warp::path("hub").and(create_hub().or(update_hub(server.clone())).or(get_hub()));
    warp::path("rest").and(hub)
}

fn api(
    server: ServerAddress,
    schema: GraphQLSchema,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    rest(Arc::clone(&server))
        .or(send_message(Arc::clone(&server)))
        .or(websocket(Arc::clone(&server)))
        .or(graphql(server, schema))
}
