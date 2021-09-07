use async_graphql::{EmptySubscription, Request as GraphQLRequest, Schema};

use serde::{Deserialize, Serialize};
use xactor::Actor;

use std::convert::Infallible;
use std::convert::TryInto;
use std::net::SocketAddr;
use std::sync::Arc;

use warp::hyper::body::Bytes;
use warp::ws::Ws;
use warp::Filter;
use warp::Reply;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::server::Server;
use crate::ID;
use crate::{
    graphql_model::{MutationRoot, QueryRoot},
    server::ServerNotification,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerInfo {
    pub version: String,
    pub key_server: String,
}

pub async fn start(config: Config) -> Result {
    let schema = Schema::build(QueryRoot, MutationRoot, EmptySubscription).finish();
    let server = Arc::new(
        Server::new()
            .await?
            .start()
            .await
            .map_err(|_| Error::ServerStartFailed)?,
    );
    let send_message_server_arc = server.clone();
    let graphql_server_arc = server.clone();
    let user_id_header =
        warp::any()
            .and(warp::header("user-id"))
            .and_then(move |header: String| async {
                let header = header;
                ID::parse_str(&header).map_err(|e| warp::reject::custom(Error::ID(e)))
            });

    let schema_sdl = schema.sdl();
    let graphql_post = warp::any()
        .and(warp::path!("v3" / "graphql"))
        .and(warp::body::bytes())
        .and(user_id_header)
        .and_then(move |body: Bytes, user_id: ID| {
            let server = graphql_server_arc.clone();
            let schema = schema.clone();
            async move {
                Ok::<_, Infallible>(
                    async {
                        let content =
                            String::from_utf8(body.to_vec()).map_err(|_| Error::InvalidText)?;
                        let request = GraphQLRequest::new(content);
                        let resp = schema.execute(request.data(server).data(user_id)).await;

                        let mut response = resp.data.to_string().into_response();
                        if let Some(value) = resp.cache_control.value() {
                            if let Ok(value) = value.try_into() {
                                response.headers_mut().insert("cache-control", value);
                            }
                        }
                        for (name, value) in resp.http_headers {
                            if let Some(name) = name {
                                if let Ok(value) = value.try_into() {
                                    response.headers_mut().append(name, value);
                                }
                            }
                        }
                        Ok::<_, Error>(response)
                    }
                    .await
                    .map_or_else(|e| e.into_response(), |r| r.into_response()),
                )
            }
        });

    let send_message = warp::any()
        .and(warp::path!("v3" / "send_message"))
        .and(warp::filters::body::bytes())
        .and_then(move |body: Bytes| {
            let server = send_message_server_arc.clone();
            async move {
                Ok::<_, Infallible>(
                    async {
                        let message_string =
                            String::from_utf8(body.to_vec()).map_err(|_| Error::InvalidText)?;
                        let message = serde_json::from_str(&message_string)?;
                        let _ = server.send(ServerNotification::NewMessage(message));
                        Ok("OK".to_owned())
                    }
                    .await
                    .map_or_else(|e: Error| e.into_response(), |r| r.into_response()),
                )
            }
        });

    let web_socket = warp::path!("v3" / "websocket")
        .and(user_id_header)
        .and(warp::ws())
        .map(move |user_id: ID, ws: Ws| {
            let server = server.clone();
            ws.on_upgrade(move |websocket| async move {
                let _ = crate::websocket::handle_connection(websocket, user_id, server).await;
            })
        });

    let server_info_struct = ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        key_server: config.key_server,
    };
    let server_info_str = serde_json::to_string(&server_info_struct).unwrap();

    let server_info =
        warp::path!("v3" / "info").map(move || server_info_str.clone().into_response());
    let graphql_schema =
        warp::path!("v3" / "graphql_schema").map(move || schema_sdl.clone().into_response());

    let cors = warp::cors().allow_any_origin();
    let log = warp::log("wicrs_server::http");

    let routes = graphql_post
        .or(server_info)
        .or(graphql_schema)
        .or(web_socket)
        .or(send_message)
        .with(cors)
        .with(log);
    let server = warp::serve(routes).run(
        config
            .address
            .parse::<SocketAddr>()
            .expect("Invalid bind address"),
    );

    server.await;

    Ok(())
}
